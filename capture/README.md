# What we found in C:\Program Files\Sublime Text\sublime_text.exe

sublime_text.exe is a program for Windows, built for x86_64 (64-bit). Its internal names were removed (stripped), so most functions will have generated names.

Evidence ev_f1fb9073c7bf8fa7bc354c787a3c3844325bea266a60f2db5a560b44e678da4c.

```json
{
  "architecture": "x86_64",
  "bits": 64,
  "dotnet_arch": null,
  "entry_point": "0x14056034c",
  "format": "pe",
  "format_name": "PE (Windows)",
  "kind": "executable",
  "libraries": [
    "ADVAPI32.dll",
    "COMCTL32.dll",
    "COMDLG32.dll",
    "GDI32.dll",
    "IMM32.dll",
    "KERNEL32.dll",
    "OPENGL32.dll",
    "SHELL32.dll",
    "USER32.dll",
    "UxTheme.dll",
    "WININET.dll",
    "dwmapi.dll",
    "ntdll.dll",
    "ole32.dll"
  ],
  "next_steps": [
    {
      "tool": "core_overview_show",
      "why": "a one-page summary: sections, imports, counts"
    },
    {
      "tool": "native_functions_list",
      "why": "every function we found"
    },
    {
      "tool": "native_strings_find",
      "why": "find text the program shows or uses"
    },
    {
      "tool": "native_function_study",
      "why": "a full dossier on one function (start with main or the entry point)"
    },
    {
      "tool": "core_feature_find",
      "why": "follow a word you see in the app to the code behind it"
    },
    {
      "tool": "native_pe_resources_list",
      "why": "icons, version information and the manifest"
    }
  ],
  "operating_system": "Windows",
  "plain_english": "sublime_text.exe is a program for Windows, built for x86_64 (64-bit). Its internal names were removed (stripped), so most functions will have generated names.",
  "recommended_toolsets": [
    "core",
    "native",
    "formats"
  ],
  "stripped": true,
  "symbol_table_stripped": true,
  "toolchain_hints": []
}
```

## Ghidra

Deep mode: Ghidra has analysed C:\Program Files\Sublime Text\sublime_text.exe (8 min), so decompiling it is quick.
