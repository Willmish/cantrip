/*
 * Copyright 2022 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/*
 * ProcessManager loads a0-a3 with _sel4_ipc_buffer and the information
 * needed to communicate with the SDKRuntime. Save those to the well-known
 * memory locations referenced by the SDK RPC.
 */
    .section .text._start
    .align 2
    .globl _start
    .type _start, @function
_start:
    .option push
    .option norelax

    la gp, __global_pointer$
    la x4, _tls                 /* setup static TLS */

    lui t1, 0
    add t1, t1, tp
    sw a0, 0(t1)                /* setup __sel4_ipc_buffer at _tls+0 */

    la t1, CANTRIP_SDK_ENDPOINT
    sw a1, 0(t1)                /* seL4_CPtr of SDKRuntime Endpoint */

    la t1, CANTRIP_SDK_FRAME
    sw a2, 0(t1)                /* seL4_CPtr of CANTRIP_SDK_PARAMS Frame object */

    la t1, CANTRIP_SDK_PARAMS
    sw a3, 0(t1)                /* virtual address of CANTRIP_SDK_PARAMS */

    .option pop

    jal main
1:  j 1b                        /* NB: should not return */

    .section .bss
    .align 12
    .globl _tls
    .type _tls, tls_object
_tls:
    .ds.b 4                     /* NB: space only for __sel4_ipc_buffer */

    .align 2
    .global CANTRIP_SDK_ENDPOINT
CANTRIP_SDK_ENDPOINT:
    .ds.b 4

    .align 2
    .global CANTRIP_SDK_FRAME
CANTRIP_SDK_FRAME:
    .ds.b 4

    .align 2
    .global CANTRIP_SDK_PARAMS
CANTRIP_SDK_PARAMS:
    .ds.b 4

    .section .tbss
    .align 2
    .globl __sel4_ipc_buffer
    .type __sel4_ipc_buffer, tls_object
__sel4_ipc_buffer:
    .zero 4
