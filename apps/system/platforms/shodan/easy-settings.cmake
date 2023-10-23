# Copyright 2020 Google LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Shodan-specific configuration.

# Carefully size the rootserver data. Peak memory use during boot is when
# the rootserver runs so we tune this and the rootserver's internal data
# structure sizes to minimize waste.
if (RELEASE)
  set(KernelRootCNodeSizeBits 11 CACHE STRING "Root CNode Size (2^n slots)")
  set(KernelMaxNumBootinfoUntypedCaps 128 CACHE STRING "Max number of bootinfo untyped caps")
else()
  # NB: for Shodan, 13 works but is tight
  set(KernelRootCNodeSizeBits 13 CACHE STRING "Root CNode Size (2^n slots)")
  set(KernelMaxNumBootinfoUntypedCaps 128 CACHE STRING "Max number of bootinfo untyped caps")
endif()
