# YARA scanning

YARA matches files against rules that describe malware families, tools or
suspicious patterns. It is useful when you know what you are looking for:
indicators from an incident report, a vendor advisory or your own analysis.

## Rules

**Sanctum ships no YARA rules.** Public rule sets come under different
licences, and some restrict use. Bring the rules you are entitled to use on
a USB stick. Curated, signed rules will arrive through Abyssal Warden's
content bundles (ROADMAP Phase 6).

## Scanning

Mount the target read-only first (see the
[filesystem guide](../recovery/filesystems.md#mounting-read-only-properly)).

```bash
yara -r /mnt/usb/rules/index.yar /mnt/target
```

| Option | Use |
| --- | --- |
| `-r` | Recurse into directories |
| `-s` | Show which strings matched, and where |
| `-w` | Hide warnings about slow rules |
| `-p 4` | Use four threads |
| `-f` | Fast mode: stop at the first match per rule |

To keep the results:

```bash
yara -r -s /mnt/usb/rules/index.yar /mnt/target > /mnt/usb/yara-results.txt
```

## Large rule sets

Compile rules once to load them faster:

```bash
yarac /mnt/usb/rules/index.yar /mnt/usb/rules/compiled.yarc
yara -C -r /mnt/usb/rules/compiled.yarc /mnt/target
```

## Reading results

A match means a file contains what the rule describes. It is not proof of
infection: generic rules match legitimate software too. Check matches by
hash (`sha256sum`) against other sources, and look at where each file is
and what loads it.
