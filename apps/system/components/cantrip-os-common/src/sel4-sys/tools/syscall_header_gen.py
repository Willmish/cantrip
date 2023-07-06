#!/usr/bin/env python3
#
# Copyright 2015, Corey Richardson
# Copyright 2014, NICTA
#
# This software may be distributed and modified according to the terms of
# the BSD 2-Clause license. Note that NO WARRANTY is provided.
# See "LICENSE_BSD2.txt" for details.
#
# @TAG(NICTA_BSD)
#

# seL4 System Call ID Generator
# ==============================

from __future__ import division, print_function
import argparse
import re
import sys
import xml.dom.minidom
import pkg_resources
# We require jinja2 to be at least version 2.10 as we use the 'namespace' feature from
# that version
pkg_resources.require("jinja2>=2.10")
from jinja2 import Environment, BaseLoader

COMMON_HEADER = """

/* This header was generated by kernel/tools/syscall_header_gen.py.
 *
 * To add a system call number, edit kernel/include/api/syscall.xml
 *
 */"""

LIBSEL4_HEADER_TEMPLATE = \
"""/* @LICENSE(NICTA) */""" + COMMON_HEADER + """
#[repr(isize)]
pub enum SyscallId {
{%- set ns = namespace(syscall_number=-1) -%}
{%- for name, condition, list in enum  -%}
    {%- if condition | length > 0 %}
    #[cfg({{condition}})]
    {%- endif  %}
    {%- for syscall in list %}
    {{syscall}} = {{ns.syscall_number}},
    {%- set ns.syscall_number = ns.syscall_number -1 -%}
    {%- endfor  %}
{%- endfor  %}
}
"""

def parse_args():
    parser = argparse.ArgumentParser(description="""Generate seL4 syscall API constants
                                                    and associated header files""")
    parser.add_argument('--xml', type=argparse.FileType('r'),
            help='Name of xml file with syscall name definitions', required=True)
    parser.add_argument('--mcs', action='store_true',
                        help='Generate MCS api')
    parser.add_argument('--dest', type=argparse.FileType('w'),
            help='Name of file to generate for librustsel4', required=True)

    result = parser.parse_args()

    return result

def parse_syscall_list(element):
    syscalls = []
    for config in element.getElementsByTagName("config"):
        condition = config.getAttribute("condition")
        # HACK: ugly hacks to handle simple CPP expressions (very fragile)
        # NB: CONFIG_MAX_NUM_NODES > 1 =>'s CONFIG_SMP_SUPPORT
        condition = condition.replace('CONFIG_MAX_NUM_NODES > 1', 'CONFIG_SMP_SUPPORT')
        if condition == "defined CONFIG_DEBUG_BUILD && CONFIG_SMP_SUPPORT":
            condition = 'all(feature = "CONFIG_DEBUG_BUILD", feature = "CONFIG_SMP_SUPPORT")'
        elif condition == "defined CONFIG_DEBUG_BUILD && defined CONFIG_BENCHMARK_TRACK_UTILISATION":
            condition = 'all(feature = "CONFIG_DEBUG_BUILD", feature = "CONFIG_BENCHMARK_TRACK_UTILISATION")'
        elif condition:
            condition = condition.replace('defined', '')
            condition = condition.replace('(', '')
            condition = condition.replace(')', '')
            condition = condition.replace(' ', '')
            if 'CONFIG_' in condition:
                condition = 'feature = "' + condition + '"'
            if '!' in condition:
                condition = 'not(%s)' % condition.replace('!', '')

        config_name = config.getAttribute("name")
        config_syscalls = []
        for syscall in config.getElementsByTagName("syscall"):
            name = str(syscall.getAttribute("name"))
            config_syscalls.append(name)
        syscalls.append((config_name, condition, config_syscalls))

    # sanity check
    assert len(syscalls) != 0

    return syscalls


def parse_xml(xml_file, mcs):
    # first check if the file is valid xml
    try:
        doc = xml.dom.minidom.parse(xml_file)
    except:
        print("Error: invalid xml file.", file=sys.stderr)
        sys.exit(-1)

    tag = "api-mcs" if mcs else "api-master"
    api = doc.getElementsByTagName(tag)
    if len(api) != 1:
        print("Error: malformed xml. Only one api element allowed",
              file=sys.stderr)
        sys.exit(-1)

    configs = api[0].getElementsByTagName("config")
    if len(configs) != 1:
        print("Error: api element only supports 1 config element",
                file=sys.stderr)
        sys.exit(-1)

    if len(configs[0].getAttribute("name")) != 0:
        print("Error: api element config only supports an empty name",
                file=sys.stderr)
        sys.exit(-1)

    # debug elements are optional
    debug = doc.getElementsByTagName("debug")
    if len(debug) != 1:
        debug_element = None
    else:
        debug_element = debug[0]

    api_elements = parse_syscall_list(api[0])
    debug = parse_syscall_list(debug_element)

    return (api_elements, debug)

def generate_libsel4_file(libsel4_header, syscalls):
    template = Environment(
        loader=BaseLoader, trim_blocks=False,
        lstrip_blocks=False).from_string(LIBSEL4_HEADER_TEMPLATE)
    data = template.render({'enum': syscalls})
    libsel4_header.write(data)

if __name__ == "__main__":
    args = parse_args()

    (api, debug) = parse_xml(args.xml, args.mcs)
    args.xml.close()

    generate_libsel4_file(args.dest, api + debug)
    args.dest.close()
