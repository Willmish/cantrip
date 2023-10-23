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

# We're not attempting to actually compile Rust files. All we want to make use
# of is the alternative language linkage behaviors in CMake.

set(CMAKE_Rust_COMPILER "/bin/false")
set(CMAKE_Rust_COMPILER_ID "Rust")
set(CMAKE_Rust_PLATFORM_ID "Rust")
set(CMAKE_Rust_COMPILER_VERSION "")

mark_as_advanced(CMAKE_Rust_COMPILER)
set(CMAKE_Rust_COMPILER_LOADED 1)

configure_file(
    "${CMAKE_CURRENT_LIST_DIR}/CMakeRustCompiler.cmake.in"
	"${CMAKE_BINARY_DIR}${CMAKE_FILES_DIRECTORY}/${CMAKE_VERSION}/CMakeRustCompiler.cmake"
    IMMEDIATE @ONLY)

# Silence CMake warnings about not setting this variable
set(CMAKE_Rust_COMPILER_ENV_VAR "")

# We don't need to test this.
set(CMAKE_Rust_COMPILER_WORKS 1 CACHE INTERNAL "")
