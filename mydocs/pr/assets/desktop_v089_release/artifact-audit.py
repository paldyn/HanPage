#!/usr/bin/env python3
"""Audit downloaded HanPage Desktop artifacts; never install or execute them.

Writes only one temporary ICO in an owned temporary directory, then removes it.
Repository and GitHub are read-only inputs. JSON is printed to stdout.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
import re
import struct
from pathlib import Path
import subprocess
import sys
import tempfile
import types

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

WINDOWS_HELPER = '#!/usr/bin/env python3\n"""Read-only PE RT_ICON/RT_GROUP_ICON audit against an ICO; never runs EXEs.\n\nPE and ICO parsing uses the standard library. Pillow is optional for decoded\nRGBA equivalence (run with Codex\'s bundled Python to enable it). No files are\nwritten by this program. Its JSON covers only the outer PE icon resources,\nnot the NSIS compressed app/uninstaller or UI/banner bitmaps.\n\nExit 0: one selected group has every expected frame, by pixels or exact payload.\nExit 1: valid resources do not match. Exit 2: invalid input or incomplete audit.\n"""\nimport argparse\nfrom collections import Counter\nfrom hashlib import sha256\nfrom io import BytesIO\nimport json\nfrom pathlib import Path\nimport struct\nimport sys\n\ntry:\n    from PIL import Image\nexcept ImportError:\n    Image = None\n\n\ndef digest(data):\n    return sha256(data).hexdigest()\n\n\ndef unpack(fmt, data, offset):\n    size = struct.calcsize(fmt)\n    if offset < 0 or offset + size > len(data):\n        raise ValueError(f"Out-of-bounds structure: offset={offset}, bytes={size}")\n    return struct.unpack_from(fmt, data, offset)\n\n\ndef frame_record(payload, width, height, planes, bits, **extra):\n    record = dict(width=width, height=height, planes=planes, bit_count=bits,\n                  payload_bytes=len(payload), payload_sha256=digest(payload),\n                  encoding="PNG" if payload.startswith(b"\\x89PNG\\r\\n\\x1a\\n") else "DIB",\n                  **extra)\n    if Image is None:\n        record["pixels_unavailable"] = "Pillow is not installed in this Python"\n        return record\n    # A one-frame ICO preserves ICO alpha/mask semantics, including DIB frames.\n    entry = struct.pack("<BBBBHHII", 0 if width == 256 else width,\n                        0 if height == 256 else height, 0, 0,\n                        planes, bits, len(payload), 22)\n    ico = struct.pack("<HHH", 0, 1, 1) + entry + payload\n    try:\n        with Image.open(BytesIO(ico)) as image:\n            image.load()\n            rgba = image.convert("RGBA")\n            record["decoded_width"], record["decoded_height"] = rgba.size\n            record["decoded_dimensions_match_directory"] = rgba.size == (width, height)\n            record["rgba_sha256"] = digest(rgba.tobytes())\n            record["rgba_hash_semantics"] = "row-major RGBA8, top-left origin, no scaling"\n    except Exception as error:\n        record["decode_error"] = f"{type(error).__name__}: {error}"\n    return record\n\n\ndef read_ico(path):\n    data = path.read_bytes()\n    reserved, kind, count = unpack("<HHH", data, 0)\n    if reserved != 0 or kind != 1 or not 1 <= count <= 256:\n        raise ValueError("Invalid ICO header/count")\n    unpack(f"<{count * 16}s", data, 6)\n    frames = []\n    for index in range(count):\n        w, h, colors, reserved, planes, bits, size, offset = unpack("<BBBBHHII", data, 6 + 16 * index)\n        if reserved != 0 or offset < 6 + 16 * count or not size or offset + size > len(data):\n            raise ValueError(f"Invalid ICO frame {index}")\n        frames.append(frame_record(data[offset:offset + size], w or 256, h or 256,\n                                   planes, bits, index=index, color_count=colors,\n                                   file_offset=offset))\n    return dict(path=str(path.resolve()), file_bytes=len(data), file_sha256=digest(data),\n                frame_count=count, frames=frames)\n\n\ndef pe_resources(path):\n    data = path.read_bytes()\n    if data[:2] != b"MZ":\n        raise ValueError("Not a DOS/PE image")\n    pe_offset, = unpack("<I", data, 0x3c)\n    if data[pe_offset:pe_offset + 4] != b"PE\\0\\0":\n        raise ValueError("Missing PE signature")\n    machine, section_count, timestamp, symbols, symbol_count, optional_size, flags = unpack("<HHIIIHH", data, pe_offset + 4)\n    optional_offset = pe_offset + 24\n    magic, = unpack("<H", data, optional_offset)\n    if magic == 0x10b:\n        count_offset, directory_offset, pe_kind = 92, 96, "PE32"\n    elif magic == 0x20b:\n        count_offset, directory_offset, pe_kind = 108, 112, "PE32+"\n    else:\n        raise ValueError(f"Unsupported PE optional-header magic {magic:#x}")\n    if optional_size < directory_offset + 24:\n        raise ValueError("PE optional header has no resource-directory slot")\n    directory_count, = unpack("<I", data, optional_offset + count_offset)\n    if directory_count < 3:\n        raise ValueError("PE has no resource data directory")\n    resource_rva, resource_size = unpack("<II", data, optional_offset + directory_offset + 16)\n    headers_size, = unpack("<I", data, optional_offset + 60)\n    sections = []\n    for index in range(section_count):\n        offset = optional_offset + optional_size + 40 * index\n        name, virtual_size, virtual_address, raw_size, raw_offset = unpack("<8sIIII", data, offset)\n        if raw_offset + raw_size > len(data):\n            raise ValueError("Section raw range extends past EOF")\n        sections.append(dict(name=name.rstrip(b"\\0").decode("ascii", "replace"),\n                             virtual_size=virtual_size, virtual_address=virtual_address,\n                             raw_size=raw_size, raw_offset=raw_offset))\n\n    def map_rva(rva, size):\n        if size < 0:\n            raise ValueError("Negative resource size")\n        if rva < headers_size and rva + size <= min(headers_size, len(data)):\n            return rva\n        for section in sections:\n            delta = rva - section["virtual_address"]\n            if 0 <= delta and delta + size <= section["raw_size"]:\n                return section["raw_offset"] + delta\n        raise ValueError(f"RVA {rva:#x} size {size} has no raw-file mapping")\n\n    if not resource_rva or resource_size < 16:\n        raise ValueError("Empty PE resource directory")\n    root = map_rva(resource_rva, resource_size)\n    leaves, active = [], set()\n\n    def resource_offset(relative, size):\n        if relative < 0 or relative + size > resource_size:\n            raise ValueError("Resource directory reference exceeds declared range")\n        return root + relative\n\n    def resource_name(value):\n        if not value & 0x80000000:\n            return value\n        relative = value & 0x7fffffff\n        length, = unpack("<H", data, resource_offset(relative, 2))\n        offset = resource_offset(relative + 2, length * 2)\n        return data[offset:offset + length * 2].decode("utf-16-le", "strict")\n\n    def walk(relative, keys):\n        if len(keys) > 8 or relative in active:\n            raise ValueError("Resource tree cycle or excessive depth")\n        active.add(relative)\n        offset = resource_offset(relative, 16)\n        characteristics, timestamp, major, minor, named, ids = unpack("<IIHHHH", data, offset)\n        count = named + ids\n        resource_offset(relative + 16, count * 8)\n        for index in range(count):\n            name_value, target = unpack("<II", data, offset + 16 + index * 8)\n            path_keys = keys + [resource_name(name_value)]\n            if target & 0x80000000:\n                walk(target & 0x7fffffff, path_keys)\n            else:\n                value_offset = resource_offset(target, 16)\n                rva, size, code_page, reserved = unpack("<IIII", data, value_offset)\n                file_offset = map_rva(rva, size)\n                leaves.append(dict(keys=path_keys, code_page=code_page, data_rva=rva,\n                                   file_offset=file_offset, payload=data[file_offset:file_offset + size]))\n        active.remove(relative)\n\n    walk(0, [])\n    info = dict(path=str(path.resolve()), file_bytes=len(data), file_sha256=digest(data),\n                pe_kind=pe_kind, machine_hex=f"0x{machine:04x}",\n                resource_rva=resource_rva, resource_size=resource_size,\n                resource_leaf_count=len(leaves), sections=sections)\n    return info, leaves\n\n\ndef multiset(frames, field):\n    if any(field not in frame for frame in frames):\n        return None\n    return Counter((f["width"], f["height"], f[field]) for f in frames)\n\n\ndef compare(source, frames):\n    payload = multiset(frames, "payload_sha256")\n    source_payload = multiset(source, "payload_sha256")\n    pixels = multiset(frames, "rgba_sha256")\n    source_pixels = multiset(source, "rgba_sha256")\n    payload_equal = payload == source_payload\n    pixels_equal = None if pixels is None or source_pixels is None else pixels == source_pixels\n    pixel_dimensions_valid = all(f.get("decoded_dimensions_match_directory", False) for f in frames + source)\n    if pixels_equal is True and not pixel_dimensions_valid:\n        pixels_equal = False\n    shared_payload = sum((payload & source_payload).values()) if payload is not None else 0\n    shared_pixels = sum((pixels & source_pixels).values()) if pixels is not None and source_pixels is not None else None\n    return dict(frame_count_matches_source=len(frames) == len(source),\n                frame_dimensions_multiset_matches_source=Counter((f["width"], f["height"]) for f in frames) == Counter((f["width"], f["height"]) for f in source),\n                payload_multiset_matches_source=payload_equal,\n                rgba_multiset_matches_source=pixels_equal,\n                matching_payload_frames=shared_payload,\n                matching_rgba_frames=shared_pixels,\n                frame_order_is_not_required=True,\n                full_source_match=payload_equal or pixels_equal is True)\n\n\ndef audit(exe_path, ico_path, selected_group):\n    expected = read_ico(ico_path)\n    pe, leaves = pe_resources(exe_path)\n    icons = [leaf for leaf in leaves if len(leaf["keys"]) == 3 and leaf["keys"][0] == 3]\n    groups = [leaf for leaf in leaves if len(leaf["keys"]) == 3 and leaf["keys"][0] == 14]\n    group_records, icon_records = [], []\n    for leaf in icons:\n        icon_records.append(dict(resource_id=leaf["keys"][1], language_id=leaf["keys"][2],\n                                 file_offset=leaf["file_offset"], payload_bytes=len(leaf["payload"]),\n                                 payload_sha256=digest(leaf["payload"]),\n                                 encoding="PNG" if leaf["payload"].startswith(b"\\x89PNG\\r\\n\\x1a\\n") else "DIB"))\n    for leaf in groups:\n        group_id, language = leaf["keys"][1:]\n        payload = leaf["payload"]\n        record = dict(group_id=group_id, language_id=language,\n                      selected=selected_group is None or str(group_id) == selected_group,\n                      directory_payload_sha256=digest(payload),\n                      file_offset=leaf["file_offset"], frames=[], errors=[])\n        try:\n            reserved, kind, count = unpack("<HHH", payload, 0)\n            if reserved != 0 or kind != 1 or not 1 <= count <= 256 or len(payload) != 6 + count * 14:\n                raise ValueError("Invalid RT_GROUP_ICON header/count/length")\n            record["frame_count"] = count\n            for index in range(count):\n                w, h, colors, reserved, planes, bits, size, resource_id = unpack("<BBBBHHIH", payload, 6 + 14 * index)\n                candidates = [i for i in icons if i["keys"][1] == resource_id and i["keys"][2] == language]\n                resolution = "same language"\n                if not candidates:\n                    candidates = [i for i in icons if i["keys"][1] == resource_id]\n                    resolution = "unique resource ID across languages"\n                if len(candidates) != 1:\n                    raise ValueError(f"RT_ICON {resource_id} resolves to {len(candidates)} entries")\n                icon = candidates[0]\n                if len(icon["payload"]) != size:\n                    raise ValueError(f"RT_ICON {resource_id} length disagrees with group")\n                record["frames"].append(frame_record(icon["payload"], w or 256, h or 256, planes, bits,\n                                                     index=index, resource_id=resource_id,\n                                                     language_id=icon["keys"][2], resolution=resolution,\n                                                     file_offset=icon["file_offset"], color_count=colors))\n            record["comparison"] = compare(expected["frames"], record["frames"])\n        except Exception as error:\n            record["errors"].append(f"{type(error).__name__}: {error}")\n        group_records.append(record)\n    selected = [record for record in group_records if record["selected"]]\n    exact_payload = any(r.get("comparison", {}).get("payload_multiset_matches_source") for r in selected)\n    exact_pixels = any(r.get("comparison", {}).get("rgba_multiset_matches_source") is True for r in selected)\n    decoding_errors = [f.get("decode_error") for f in expected["frames"] if f.get("decode_error")]\n    decoding_errors += [f.get("decode_error") for r in selected for f in r["frames"] if f.get("decode_error")]\n    structural_errors = [error for r in selected for error in r["errors"]]\n    if not selected:\n        status, exit_code = "incomplete_no_selected_icon_group", 2\n    elif structural_errors or decoding_errors:\n        status, exit_code = "incomplete_audit_error", 2\n    elif exact_payload or exact_pixels:\n        status, exit_code = "expected_icon_present", 0\n    elif Image is None:\n        status, exit_code = "incomplete_pixels_unavailable_and_payloads_differ", 2\n    else:\n        status, exit_code = "expected_icon_absent", 1\n    report = dict(schema="hanpage.outer-pe-icon-audit.v1", status=status, exit_code=exit_code,\n                  expected_ico=expected, exe=pe,\n                  pillow_available=Image is not None,\n                  pillow_version=None if Image is None else __import__("PIL").__version__,\n                  scope="Outer PE RT_ICON/RT_GROUP_ICON only; no installation or EXE execution; NSIS compressed app/uninstaller and banner bitmaps not inspected",\n                  selected_group_id=selected_group,\n                  rt_icon_count=len(icons), rt_group_icon_count=len(groups),\n                  rt_icons=icon_records, groups=group_records,\n                  selected_any_group_payload_matches_source=exact_payload,\n                  selected_any_group_rgba_matches_source=exact_pixels,\n                  all_selected_groups_match_source=bool(selected) and all(r.get("comparison", {}).get("full_source_match", False) for r in selected),\n                  audit_errors=structural_errors + decoding_errors)\n    return report, exit_code\n\n\ndef main():\n    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)\n    parser.add_argument("--exe", required=True, type=Path, help="Read-only installer/PE image path")\n    parser.add_argument("--ico", required=True, type=Path, help="Expected original/active ICO path")\n    parser.add_argument("--group-id", help="Select a numeric or named RT_GROUP_ICON ID (otherwise audit all groups)")\n    args = parser.parse_args()\n    try:\n        report, code = audit(args.exe, args.ico, args.group_id)\n    except Exception as error:\n        report, code = dict(schema="hanpage.outer-pe-icon-audit.v1", status="input_or_parser_error",\n                            exe_path=str(args.exe.resolve()), ico_path=str(args.ico.resolve()),\n                            error=f"{type(error).__name__}: {error}", exit_code=2), 2\n    print(json.dumps(report, ensure_ascii=False, indent=2))\n    return code\n\n\nif __name__ == "__main__":\n    sys.exit(main())\n'

EXPECTED_KEY_ID = '835d6b3831e133aa'
EXPECTED_KEY_TEXT_SHA256 = '90046ede9558e71ec15280e8ee1d25594e302b74c9f33d4cb91ac16664997141'
EXPECTED_ICO_SHA256 = 'c4bdbc5ab66b05b549c6c4113b030133c1691450c68bb8183232b516efde2b09'
EXPECTED_ICNS_SHA256 = '8260b6100180a8c589c5b63943d77587fc575bbc74d591d3249557a6df8d70c4'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def git_blob(repo, source, path):
    return subprocess.check_output(['git', '-C', str(repo), 'show', f'{source}:{path}'])


def minisign_verify(data, key_outer, sig_outer):
    decode = lambda item: base64.b64decode(item, validate=True)
    key_text = decode(key_outer)
    key_lines = key_text.decode('utf-8').splitlines()
    sig_lines = decode(sig_outer).decode('utf-8').splitlines()
    require(len(key_lines) == 2 and len(sig_lines) == 4, 'Unexpected minisign text layout')
    require(key_lines[0].startswith('untrusted comment: ') and sig_lines[0].startswith('untrusted comment: '), 'Missing minisign untrusted comment')
    kp, sp, global_sig = decode(key_lines[1]), decode(sig_lines[1]), decode(sig_lines[3])
    require(len(kp) == 42 and len(sp) == 74 and len(global_sig) == 64, 'Unexpected minisign packet size')
    require(kp[:2] in (b'Ed', b'ED') and sp[:2] in (b'Ed', b'ED'), 'Unsupported minisign algorithm')
    require(kp[2:10] == sp[2:10] and kp[2:10].hex() == EXPECTED_KEY_ID, 'Updater key identity changed')
    require(sha(key_text) == EXPECTED_KEY_TEXT_SHA256, 'Updater public-key text changed')
    require(sig_lines[2].startswith('trusted comment: '), 'Missing minisign trusted comment')
    trusted_comment = sig_lines[2][len('trusted comment: '):].encode('utf-8')
    # ED signs a BLAKE2b-512 prehash; Ed signs the original payload.
    message = hashlib.blake2b(data, digest_size=64).digest() if sp[:2] == b'ED' else data
    key = Ed25519PublicKey.from_public_bytes(kp[10:])
    key.verify(sp[10:], message)
    # The global signature covers only the 64-byte data signature + comment.
    key.verify(global_sig, sp[10:] + trusted_comment)
    return dict(algorithm=sp[:2].decode(), key_id_hex=kp[2:10].hex(),
                trusted_comment=trusted_comment.decode('utf-8'),
                public_key_text_sha256=sha(key_text), payload_signature_verified=True,
                global_signature_verified=True)



def parse_version_info(payload):
    """Decode a bounded RT_VERSION tree with DWORD alignment and UTF-16LE keys.

    MS VS_VERSIONINFO values count bytes; String values count WCHAR words.
    Structural StringFileInfo/StringTable values have zero length.
    """
    def align4(offset):
        return (offset + 3) & ~3

    parsed_nodes = 0

    def block(offset, limit, depth=0):
        nonlocal parsed_nodes
        parsed_nodes += 1
        require(depth <= 12 and parsed_nodes <= 2048, 'Version resource nesting/node limit exceeded')
        require(offset + 6 <= limit, 'Truncated VERSIONINFO block header')
        length, value_length, value_type = struct.unpack_from('<HHH', payload, offset)
        end = offset + length
        require(length >= 8 and end <= limit, 'VERSIONINFO block length exceeds its parent')
        require(value_type in (0, 1), 'Unexpected VERSIONINFO value type')
        key_start, cursor = offset + 6, offset + 6
        while cursor + 2 <= end and payload[cursor:cursor + 2] != b'\0\0':
            cursor += 2
        require(cursor + 2 <= end, 'VERSIONINFO key has no UTF-16 terminator')
        key = payload[key_start:cursor].decode('utf-16-le', 'strict')
        value_start = align4(cursor + 2)
        size = value_length * (2 if value_type == 1 else 1)
        if size == 0 and value_start > end:
            value_start = end
        require(value_start + size <= end, 'VERSIONINFO value exceeds its block')
        value = payload[value_start:value_start + size]
        value_text = value.decode('utf-16-le', 'strict').rstrip('\0') if value_type == 1 and value else None
        children = []
        child_cursor = align4(value_start + size)
        while child_cursor < end:
            tail = payload[child_cursor:end]
            if not tail.strip(b'\0'):
                break
            child = block(child_cursor, end, depth + 1)
            children.append(child)
            child_cursor = align4(child['end'])
        return dict(key=key, value=value, value_text=value_text, type=value_type,
                    offset=offset, end=end, bytes=length, children=children)

    root = block(0, len(payload))
    require(root['key'] == 'VS_VERSION_INFO' and root['type'] == 0, 'RT_VERSION root is not VS_VERSION_INFO')
    require(not payload[root['end']:].strip(b'\0'), 'Nonpadding bytes after VERSIONINFO root')
    fixed = None
    if root['value']:
        require(len(root['value']) == 52, 'VS_FIXEDFILEINFO is not 52 bytes')
        fields = struct.unpack('<13I', root['value'])
        require(fields[0] == 0xFEEF04BD, 'Invalid VS_FIXEDFILEINFO signature')
        def components(hi, lo):
            return [hi >> 16, hi & 0xffff, lo >> 16, lo & 0xffff]
        fixed = dict(struct_version_hex=f'{fields[1]:08x}',
                     file_version_components=components(fields[2], fields[3]),
                     product_version_components=components(fields[4], fields[5]),
                     file_flags_mask_hex=f'{fields[6]:08x}',
                     file_flags_hex=f'{fields[7]:08x}',
                     file_os_hex=f'{fields[8]:08x}', file_type=fields[9])
        fixed['file_version'] = '.'.join(map(str, fixed['file_version_components']))
        fixed['product_version'] = '.'.join(map(str, fixed['product_version_components']))
    tables = []
    for group in root['children']:
        if group['key'] != 'StringFileInfo':
            continue
        for table in group['children']:
            require(re.fullmatch(r'[0-9a-fA-F]{8}', table['key']) is not None, 'Unexpected StringTable language/codepage key')
            strings = {}
            for item in table['children']:
                require(item['type'] == 1 and not item['children'], 'Unexpected String value shape')
                require(item['key'] not in strings, 'Duplicate VERSIONINFO string in one table')
                strings[item['key']] = '' if item['value_text'] is None else item['value_text']
            tables.append(dict(language_codepage_hex=table['key'],
                               language_id_hex=table['key'][:4],
                               codepage_hex=table['key'][4:], strings=strings))
    return dict(root_bytes=root['bytes'], fixed_info=fixed, string_tables=tables)


def audit_windows_version(helper, exe_path, expected_version):
    """Optional metadata audit; absent fields are explicitly unverified.

    Present CompanyName/ProductVersion/FileVersion fields must match. Versions
    accept the stable 3-component app version or its equivalent '.0' form.
    """
    pe, leaves = helper.pe_resources(exe_path)
    version_leaves = [leaf for leaf in leaves if len(leaf['keys']) == 3 and leaf['keys'][0] == 16]
    records, mismatches, parse_errors, observed = [], [], [], []
    expected_components = tuple(map(int, expected_version.split('.')))
    require(len(expected_components) == 3, 'Expected semantic version must have three components')
    wanted = ('CompanyName', 'ProductVersion', 'FileVersion')
    for leaf in version_leaves:
        record = dict(resource_id=leaf['keys'][1], language_id=leaf['keys'][2],
                      file_offset=leaf['file_offset'], bytes=len(leaf['payload']),
                      payload_sha256=sha(leaf['payload']))
        try:
            parsed = parse_version_info(leaf['payload'])
            record.update(parsed)
            if parsed['fixed_info'] is not None:
                fixed = parsed['fixed_info']
                fixed['file_version_matches_expected'] = tuple(fixed['file_version_components']) == expected_components + (0,)
                fixed['product_version_matches_expected'] = tuple(fixed['product_version_components']) == expected_components + (0,)
                for field, match_field, value_field in [('FixedFileVersion', 'file_version_matches_expected', 'file_version'), ('FixedProductVersion', 'product_version_matches_expected', 'product_version')]:
                    if not fixed[match_field]:
                        mismatches.append(dict(resource_id=leaf['keys'][1], language_id=leaf['keys'][2],
                                               field=field, present=True, observed=fixed[value_field],
                                               expected=expected_version + '.0', verdict='FAIL',
                                               matches_expected=False))
            record['field_checks'] = []
            for table in parsed['string_tables']:
                for name in wanted:
                    value = table['strings'].get(name)
                    check = dict(field=name, language_codepage_hex=table['language_codepage_hex'],
                                 present=value is not None, observed=value,
                                 expected='PALDYN' if name == 'CompanyName' else expected_version)
                    if value is None:
                        check['verdict'] = 'UNVERIFIED_FIELD_ABSENT'
                    elif name == 'CompanyName':
                        check['matches_expected'] = value == 'PALDYN'
                        check['verdict'] = 'PASS' if check['matches_expected'] else 'FAIL'
                    else:
                        match = re.fullmatch(r'(\d+)\.(\d+)\.(\d+)(?:\.0)?', value)
                        check['matches_expected'] = match is not None and tuple(map(int, match.groups())) == expected_components
                        check['verdict'] = 'PASS' if check['matches_expected'] else 'FAIL'
                    record['field_checks'].append(check)
                    if check['present']:
                        observed.append(check)
                    if check['verdict'] == 'FAIL':
                        mismatches.append(dict(resource_id=leaf['keys'][1], language_id=leaf['keys'][2], **check))
        except (ValueError, AssertionError, UnicodeError, struct.error) as error:
            record['parse_error'] = f'{type(error).__name__}: {error}'
            parse_errors.append(record['parse_error'])
        records.append(record)
    observed_fields = sorted({check['field'] for check in observed})
    missing = sorted(set(wanted) - set(observed_fields))
    if mismatches:
        status = 'FAIL_PRESENT_FIELD_MISMATCH'
    elif parse_errors:
        status = 'UNVERIFIED_PARSE_ERROR'
    elif not version_leaves:
        status = 'UNVERIFIED_RESOURCE_ABSENT'
    elif missing or any(check['verdict'] == 'UNVERIFIED_FIELD_ABSENT' for record in records for check in record.get('field_checks', [])):
        status = 'PARTIAL_FIELDS_UNVERIFIED'
    else:
        status = 'PASS'
    return dict(schema='hanpage.outer-pe-version-info-audit.v1', status=status,
                exe_sha256=pe['file_sha256'], rt_version_count=len(version_leaves),
                expected_company='PALDYN', expected_version=expected_version,
                observed_fields=observed_fields, missing_fields=missing,
                resources=records, present_field_mismatches=mismatches,
                parse_errors=parse_errors,
                all_three_requested_fields_verified=status == 'PASS',
                scope='Outer installer PE RT_VERSION only; compressed app/uninstaller metadata and installed GUI/publisher screens were not inspected.',
                spec_sources=['https://learn.microsoft.com/en-us/windows/win32/menurc/vs-versioninfo',
                              'https://learn.microsoft.com/en-us/windows/win32/menurc/string-str',
                              'https://learn.microsoft.com/en-us/windows/win32/menurc/stringtable'])

def audit(args):
    version, source = args.version, args.source_sha
    folder, repo = args.release_dir.resolve(), args.repo.resolve()
    release = json.loads(args.metadata.read_text())
    tag = 'hanpage-desktop-v' + version
    require(release['tag_name'] == tag, 'Release tag does not match requested version')
    require(release['draft'] is True and release['prerelease'] is False, 'Prepublication audit requires a stable draft')
    require(release['target_commitish'] == source, 'Draft source SHA differs from verified merge SHA')
    # GitHub exposes stable draft assets under its temporary untagged identity.
    # The updater manifest must still use the final release tag, checked below.
    draft_identity = release['html_url'].rsplit('/', 1)[-1]
    require(release['html_url'].startswith('https://github.com/paldyn/HanPage/releases/tag/'), 'Unexpected draft repository URL')
    require(draft_identity == tag or (draft_identity.startswith('untagged-') and
            len(draft_identity[9:]) == 20 and all(c in '0123456789abcdef' for c in draft_identity[9:])), 'Unexpected draft identity')
    resolved = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', source + '^{commit}'], text=True).strip()
    require(resolved == source, 'Source must be a full local commit SHA')
    if args.require_tag:
        tagged = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', tag + '^{commit}'], text=True).strip()
        require(tagged == source, 'Local release tag does not peel to verified merge SHA')

    config = json.loads(git_blob(repo, source, 'HanPage-Desktop/src-tauri/tauri.conf.json'))
    require(config['version'] == version, 'Committed Tauri version differs from release version')
    require(config['identifier'] == 'com.paldyn.hanpage', 'Bundle identifier changed')
    require(config['bundle']['publisher'] == 'PALDYN', 'PALDYN publisher missing from committed source')
    require(config['bundle']['createUpdaterArtifacts'] is True, 'Updater artifacts disabled')
    require(config['plugins']['updater']['endpoints'] == ['https://github.com/paldyn/HanPage/releases/latest/download/latest.json'], 'Updater endpoint changed')
    pubkey = config['plugins']['updater']['pubkey']
    ico = git_blob(repo, source, 'HanPage-Desktop/src-tauri/icons/icon.ico')
    icns = git_blob(repo, source, 'HanPage-Desktop/src-tauri/icons/icon.icns')
    require(sha(ico) == EXPECTED_ICO_SHA256 and sha(icns) == EXPECTED_ICNS_SHA256, 'Approved branding inputs changed')

    names = {f'HanPage_{version}_aarch64.dmg', f'HanPage_{version}_x64-setup.exe',
             f'HanPage_{version}_x64-setup.exe.sig', 'HanPage_aarch64.app.tar.gz',
             'HanPage_aarch64.app.tar.gz.sig', 'latest.json'}
    assets = release['assets']
    require(len(assets) == 6 and {a['name'] for a in assets} == names, 'Draft does not contain exactly the six expected assets')
    asset_records = []
    for asset in assets:
        name = asset['name']
        require(asset['state'] == 'uploaded', f'Asset upload incomplete: {name}')
        require(asset['browser_download_url'] == f'https://github.com/paldyn/HanPage/releases/download/{draft_identity}/{name}', f'Unexpected draft asset URL: {name}')
        payload = (folder / name).read_bytes()
        require(len(payload) > 0 and len(payload) == asset['size'], f'Asset size differs from API: {name}')
        digest = sha(payload)
        require(asset.get('digest') == 'sha256:' + digest, f'Asset digest missing/different: {name}')
        asset_records.append(dict(name=name, bytes=len(payload), sha256=digest,
                                  api_digest=asset['digest'], size_matches_api=True,
                                  sha256_matches_api=True, draft_browser_download_url=asset['browser_download_url'],
                                  expected_public_url=f'https://github.com/paldyn/HanPage/releases/download/{tag}/{name}'))

    manifest_bytes = (folder / 'latest.json').read_bytes()
    manifest = json.loads(manifest_bytes)
    require(manifest['version'] == version, 'Manifest version differs from source/release')
    platforms = manifest['platforms']
    keys = {'darwin-aarch64', 'darwin-aarch64-app', 'windows-x86_64', 'windows-x86_64-nsis'}
    require(set(platforms) == keys, 'Manifest must contain exactly four platform keys')
    require(isinstance(manifest.get('notes'), str), 'Manifest release notes missing')
    date = datetime.fromisoformat(manifest['pub_date'].replace('Z', '+00:00'))
    require(date.tzinfo is not None, 'Manifest publication timestamp has no timezone')
    signature_records = []
    for platform, alias, name in [
        ('darwin-aarch64', 'darwin-aarch64-app', 'HanPage_aarch64.app.tar.gz'),
        ('windows-x86_64', 'windows-x86_64-nsis', f'HanPage_{version}_x64-setup.exe'),
    ]:
        item, alias_item = platforms[platform], platforms[alias]
        expected_url = f'https://github.com/paldyn/HanPage/releases/download/{tag}/{name}'
        require(item['url'] == alias_item['url'] == expected_url, f'Primary/alias URL differs: {platform}')
        data = (folder / name).read_bytes()
        sig = (folder / (name + '.sig')).read_text().strip()
        require(item['signature'] == alias_item['signature'] == sig, f'Primary/alias/SIG signature differs: {platform}')
        result = minisign_verify(data, pubkey, sig)
        try:
            minisign_verify(data[:-1] + bytes([data[-1] ^ 1]), pubkey, sig)
        except InvalidSignature:
            negative = True
        else:
            raise AssertionError(f'Tampered updater payload was accepted: {platform}')
        signature_records.append(dict(platform=platform, alias=alias, asset=name,
                                      bytes=len(data), sha256=sha(data), signature_verified=True,
                                      tampered_payload_rejected=negative, **result))

    helper = types.ModuleType('hanpage_outer_pe_icon_helper')
    exec(compile(WINDOWS_HELPER, '<embedded-outer-pe-icon-audit>', 'exec'), helper.__dict__)
    require(helper.Image is not None, 'Pillow required for independent unscaled RGBA frame comparison')
    with tempfile.TemporaryDirectory(prefix='hanpage089-audit-', dir='/private/tmp') as scratch:
        expected_icon = Path(scratch) / 'committed-source-icon.ico'
        expected_icon.write_bytes(ico)
        windows, exit_code = helper.audit(folder / f'HanPage_{version}_x64-setup.exe', expected_icon, None)
    require(exit_code == 0, 'Windows outer PE icon audit failed')
    require(windows['selected_any_group_payload_matches_source'] is True and
            windows['selected_any_group_rgba_matches_source'] is True and
            windows['all_selected_groups_match_source'] is True,
            'Windows installer does not contain all approved encoded/RGBA frames')
    require(windows['expected_ico']['frame_count'] == 7, 'Approved Windows ICO must contain seven frames')
    windows['expected_ico']['path'] = 'git:' + source + ':HanPage-Desktop/src-tauri/icons/icon.ico'
    windows_version = audit_windows_version(helper, folder / f'HanPage_{version}_x64-setup.exe', version)

    return dict(schema='hanpage.desktop-prepublication-artifact-audit.v1',
                checked_at_utc=datetime.now(timezone.utc).isoformat(),
                version=version, source_commit=source, tag=tag,
                release_id=release['id'], draft=True, prerelease=False,
                assets=asset_records, all_six_assets_complete=True,
                manifest_sha256=sha(manifest_bytes), manifest_bytes=len(manifest_bytes),
                manifest_platform_keys=sorted(keys), all_four_aliases_match=True,
                updater_payloads=signature_records, windows_outer_pe_icon=windows,
                windows_outer_pe_version_info=windows_version,
                source_ico_sha256=sha(ico), source_icns_sha256=sha(icns),
                committed_publisher='PALDYN', updater_endpoint_unchanged=True,
                embedded_icon_verifier_sha256=sha(WINDOWS_HELPER.encode()),
                limits=['Mac codesign/Gatekeeper/stapler/DMG must be audited separately on the new artifact.',
                        'No EXE execution, installation, app launch, updater install, or restart was performed.',
                        'Windows icon audit covers outer PE RT_ICON/RT_GROUP_ICON only; compressed app/uninstaller and publisher UI were not inspected.'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release-dir', type=Path, required=True)
    parser.add_argument('--metadata', type=Path, required=True, help='Full gh api releases/ID response JSON for the stable draft')
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--source-sha', required=True, help='Exact full verified merge commit SHA')
    parser.add_argument('--version', default='0.8.9')
    parser.add_argument('--require-tag', action='store_true', help='Also verify local tag peels to source SHA')
    args = parser.parse_args()
    try:
        result = audit(args)
    except Exception as error:
        print(json.dumps(dict(schema='hanpage.desktop-prepublication-artifact-audit.v1',
                              version=args.version, source_commit=args.source_sha,
                              status='FAILED', error_type=type(error).__name__,
                              error=str(error)), ensure_ascii=False, indent=2))
        return 1
    metadata_mismatch = result['windows_outer_pe_version_info']['status'] == 'FAIL_PRESENT_FIELD_MISMATCH'
    result['status'] = 'FAIL' if metadata_mismatch else 'PASS'
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 1 if metadata_mismatch else 0


if __name__ == '__main__':
    sys.exit(main())
