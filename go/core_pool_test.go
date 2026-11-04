package hypercast

// The primary and the pool, portable like core_test.go: TinyGo runs these too.

import (
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/SkunkWerkx/HyperCast/go/internal/core"
)

func TestCallsRunWhileThePrimaryIsHeld(t *testing.T) {
	base := withFreshInstances(t) // the pool is empty: nothing but a new instance can serve
	p := primary()
	p.mu.Lock()
	locked := true
	var wg sync.WaitGroup
	t.Cleanup(func() {
		if locked {
			p.mu.Unlock()
		}
		wg.Wait()
	})
	done := make(chan *Fault, 1)
	wg.Add(1)
	go func() {
		defer wg.Done()
		_, fault := I32("1,234", Invariant)
		done <- fault
	}()
	select {
	case fault := <-done:
		if fault != nil {
			t.Fatal(fault)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("I32 did not return while the primary was held: no pooled instance served it")
	}
	p.mu.Unlock()
	locked = false
	if d := instancesCreated.Load() - base; d != 1 {
		t.Fatalf("instances created while the primary was held: %d, want 1", d)
	}
}

func TestUncontendedCallsStayOnThePrimary(t *testing.T) {
	base := withFreshInstances(t)
	old := runtime.GOMAXPROCS(1)
	t.Cleanup(func() { runtime.GOMAXPROCS(old) })
	for range 1000 {
		if _, fault := F64("1.5", Invariant); fault != nil {
			t.Fatal(fault)
		}
	}
	if d := instancesCreated.Load() - base; d != 0 {
		t.Fatalf("uncontended calls created %d instances, want 0", d)
	}
}

// A long text's one-off instance never reaches the pool: a call with the primary held
// afterwards must make a base-sized instance of its own rather than find the one-off one.
func TestAOneOffInstanceIsNotPooled(t *testing.T) {
	base := withFreshInstances(t)
	p := primary()
	p.mu.Lock()
	defer p.mu.Unlock()
	if _, fault := I32(strings.Repeat(" ", baseText)+"1", Invariant); fault != nil {
		t.Fatal(fault)
	}
	if d := instancesCreated.Load() - base; d != 1 {
		t.Fatalf("a long text created %d instances, want 1", d)
	}
	in, how := acquire()
	defer release(in, how)
	if how != leasePooled || in.pages != basePages {
		t.Fatalf("with the primary held, acquire returned lease %d with %d pages", how, in.pages)
	}
}

// A call that panics inside the core abandons its instance: the primary stays locked and the
// next call is served from the pool, never by the instance the panic left mid-call.
func TestAPanicInsideTheCoreAbandonsThePrimary(t *testing.T) {
	base := withFreshInstances(t)
	p := primary()
	real := castDetailed
	castDetailed = func(*core.Module, int32, int32, int32, int32, int32, int32, int32, int32, int32, int32, int32) int32 {
		panic("trap")
	}
	func() {
		defer func() { castDetailed = real }()
		defer func() {
			if recover() == nil {
				t.Fatal("no panic")
			}
		}()
		Bool("yes")
	}()
	if p.mu.TryLock() {
		p.mu.Unlock()
		t.Fatal("the primary was handed back after a panic inside the core")
	}
	if v, fault := Bool("yes"); fault != nil || !v {
		t.Fatalf("Bool after the panic = %v, %v", v, fault)
	}
	if d := instancesCreated.Load() - base; d != 1 {
		t.Fatalf("after the panic: %d instances created, want 1 (the pooled one)", d)
	}
	p.mu.Unlock() // withFreshInstances restores the real primary; this one is dropped
}
