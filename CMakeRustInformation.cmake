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

include(CMakeLanguageInformation)

# We just want to use C's linkages, but without CAmkES' extra junk like the crt0
# pre- and postamble. So we're essentially bastardizing CMake's language support
# infrastructure to allow for linking Rust executables without C runtimes.
set(CMAKE_Rust_LINK_EXECUTABLE
  "<CMAKE_C_COMPILER> <FLAGS> <CMAKE_C_LINK_FLAGS> <LINK_FLAGS> <OBJECTS> <LINK_LIBRARIES> -o <TARGET>")

set(CMAKE_Rust_INFORMATION_LOADED 1)
