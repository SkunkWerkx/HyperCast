/* The ABI probe release_guard.py (in the Forge) compiles against the Swift bundle's
 * hypercast.h and links with one committed payload at a time, so each process loads or
 * links exactly one library. EXPECTED_VERSION arrives on the command line as the packed
 * MAJOR << 16 | MINOR << 8 | PATCH that hypercast_version() returns. A non-zero exit is a
 * mismatch between the header, the library and the version being released.
 *
 * The body is HyperCast's own: the version, then the result-code and fault-layout contract
 * through a boolean door (an accepted value, a refusal that must report a fault span) and an
 * integer door (an accepted value, an empty input refused with code 1), which is the shape
 * every binding declares by hand against this header. */
#include <stdint.h>
#include <string.h>
#include "hypercast.h"
int main(void) {
    uint32_t fault[2]={0,0}; uint8_t boolean=0; int32_t integer=0;
    if(hypercast_version()!=EXPECTED_VERSION) return 1;
    if(cast_bool((const uint8_t*)"yes",3,&boolean,fault)!=0 || boolean!=1) return 2;
    if(cast_bool((const uint8_t*)"maybe",5,&boolean,fault)!=2 || fault[0]!=0 || fault[1]!=5) return 3;
    if(cast_i32((const uint8_t*)"123",3,0,&integer,fault)!=0 || integer!=123) return 4;
    if(cast_i32((const uint8_t*)"",0,0,&integer,fault)!=1) return 5;
    return 0;
}
