param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [switch]$FailOnMissingImports,
    [switch]$RejectModernDpiImports
)

# Read the PE and inspect trusted system DLLs. The target executable is never run.
# Compatible with Windows PowerShell 5.1; no Python, SDK utilities or admin rights.
$ErrorActionPreference = 'Stop'
if (-not [Environment]::Is64BitProcess) { throw 'Run this script in 64-bit PowerShell.' }
$exe = (Resolve-Path -LiteralPath $Executable).Path
if (-not ('Rinva.StartupProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace Rinva {
    public sealed class Import {
        public string Dll;
        public string Name;
        public ushort Ordinal;
        public bool ByOrdinal;
        public override string ToString() { return Dll + "!" + (ByOrdinal ? "#" + Ordinal : Name); }
    }
    public sealed class PeImage {
        readonly byte[] bytes;
        readonly int sections, count, directories;
        readonly bool wide;
        public PeImage(string path) {
            bytes = File.ReadAllBytes(path);
            if (U16(0) != 0x5a4d) throw new InvalidDataException("Not a Windows executable.");
            int pe = checked((int)U32(0x3c));
            if (U32(pe) != 0x4550 || U16(pe + 4) != 0x8664)
                throw new InvalidDataException("Expected a Windows x64 executable.");
            count = U16(pe + 6);
            int optional = pe + 24;
            wide = U16(optional) == 0x20b;
            if (!wide) throw new InvalidDataException("Expected PE32+.");
            directories = optional + 112;
            sections = optional + U16(pe + 20);
        }
        ushort U16(int at) { return BitConverter.ToUInt16(bytes, at); }
        uint U32(int at) { return BitConverter.ToUInt32(bytes, at); }
        ulong U64(int at) { return BitConverter.ToUInt64(bytes, at); }
        int Offset(uint rva) {
            for (int i = 0; i < count; i++) {
                int s = sections + i * 40;
                uint start = U32(s + 12), size = U32(s + 16);
                if (rva >= start && (ulong)rva - start < size)
                    return checked((int)(U32(s + 20) + rva - start));
            }
            throw new InvalidDataException("Invalid PE virtual address.");
        }
        string CString(int at) {
            int end = at;
            while (end < bytes.Length && end - at < 4096 && bytes[end] != 0) end++;
            if (end == bytes.Length || end - at == 4096) throw new InvalidDataException("Invalid PE string.");
            return Encoding.ASCII.GetString(bytes, at, end - at);
        }
        public Import[] Imports() {
            var result = new List<Import>();
            uint rva = U32(directories + 8);
            if (rva == 0) return result.ToArray();
            int table = Offset(rva);
            for (int n = 0; n < 1024; n++) {
                int descriptor = table + n * 20;
                uint lookup = U32(descriptor), name = U32(descriptor + 12), first = U32(descriptor + 16);
                if (lookup == 0 && name == 0 && first == 0) return result.ToArray();
                string dll = CString(Offset(name));
                if (dll.IndexOfAny(new char[] { '/', '\\', ':' }) >= 0)
                    throw new InvalidDataException("Import must name a DLL, not a path.");
                int thunks = Offset(lookup == 0 ? first : lookup);
                bool terminated = false;
                for (int i = 0; i < 65536; i++) {
                    ulong entry = U64(thunks + i * 8);
                    if (entry == 0) { terminated = true; break; }
                    bool ordinal = (entry & 0x8000000000000000UL) != 0;
                    result.Add(new Import { Dll = dll, ByOrdinal = ordinal, Ordinal = (ushort)(entry & 0xffff),
                        Name = ordinal ? null : CString(Offset(checked((uint)entry)) + 2) });
                }
                if (!terminated) throw new InvalidDataException("Unterminated import table.");
            }
            throw new InvalidDataException("Too many PE import descriptors.");
        }
        uint ResourceEntry(int at, int id) {
            int entries = U16(at + 12) + U16(at + 14);
            for (int i = 0; i < entries; i++) {
                int entry = at + 16 + i * 8;
                if (id < 0 || U32(entry) == (uint)id) return U32(entry + 4);
            }
            throw new InvalidDataException("Embedded application manifest (resource 24/1) is missing.");
        }
        public string Manifest() {
            uint resource = U32(directories + 16);
            if (resource == 0) throw new InvalidDataException("No embedded Windows resources.");
            int root = Offset(resource), at = root;
            foreach (int id in new int[] { 24, 1 }) {
                uint entry = ResourceEntry(at, id);
                if ((entry & 0x80000000) == 0) throw new InvalidDataException("Invalid manifest resource tree.");
                at = root + checked((int)(entry & 0x7fffffff));
            }
            uint data = ResourceEntry(at, -1);
            if ((data & 0x80000000) != 0) throw new InvalidDataException("Invalid manifest language entry.");
            int leaf = root + checked((int)data);
            int start = Offset(U32(leaf)), length = checked((int)U32(leaf + 4));
            return Encoding.UTF8.GetString(bytes, start, length).TrimStart('\uFEFF').TrimEnd('\0');
        }
    }
    public static class StartupProbe {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        public struct ActCtx {
            public uint Size, Flags;
            public string Source;
            public ushort ProcessorArchitecture, Language;
            public string AssemblyDirectory;
            public IntPtr ResourceName;
            public string ApplicationName;
            public IntPtr Module;
        }
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern IntPtr CreateActCtxW(ref ActCtx context);
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool ActivateActCtx(IntPtr context, out UIntPtr cookie);
        [DllImport("kernel32.dll")]
        public static extern bool DeactivateActCtx(uint flags, UIntPtr cookie);
        [DllImport("kernel32.dll")]
        public static extern void ReleaseActCtx(IntPtr context);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern IntPtr LoadLibraryExW(string path, IntPtr file, uint flags);
        [DllImport("kernel32.dll", CharSet = CharSet.Ansi, ExactSpelling = true, SetLastError = true)]
        public static extern IntPtr GetProcAddress(IntPtr module, string name);
        [DllImport("kernel32.dll", EntryPoint = "GetProcAddress", ExactSpelling = true)]
        static extern IntPtr GetOrdinal(IntPtr module, IntPtr ordinal);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
        static extern uint GetModuleFileNameW(IntPtr module, StringBuilder path, int length);
        [DllImport("kernel32.dll")]
        public static extern bool FreeLibrary(IntPtr module);
        [DllImport("kernel32.dll")]
        public static extern uint SetErrorMode(uint mode);
        public static IntPtr Context(string exe) {
            var context = new ActCtx { Size = (uint)Marshal.SizeOf(typeof(ActCtx)), Flags = 8,
                Source = exe, ResourceName = new IntPtr(1) };
            IntPtr handle = CreateActCtxW(ref context);
            if (handle == new IntPtr(-1)) throw new Win32Exception(Marshal.GetLastWin32Error());
            return handle;
        }
        public static bool HasImport(IntPtr module, Import import) {
            return (import.ByOrdinal ? GetOrdinal(module, new IntPtr(import.Ordinal)) : GetProcAddress(module, import.Name)) != IntPtr.Zero;
        }
        public static string ModulePath(IntPtr module) {
            var path = new StringBuilder(32768);
            GetModuleFileNameW(module, path, path.Capacity);
            return path.ToString();
        }
    }
}
'@
}

$os = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
Write-Output "Windows: $($os.ProductName), $($os.DisplayVersion), build $($os.CurrentBuild).$($os.UBR)"
Write-Output "Executable: $exe"
$pe = [Rinva.PeImage]::new($exe)
$imports = $pe.Imports()
$manifest = [xml]$pe.Manifest()
$controls = $manifest.SelectSingleNode("//*[local-name()='assemblyIdentity' and @name='Microsoft.Windows.Common-Controls' and @version='6.0.0.0']")
if ($null -eq $controls) { throw 'The executable is missing its Common Controls v6 dependency.' }
Write-Output 'Embedded Common Controls v6 manifest: OK'
$modern = @($imports | Where-Object { $_.Name -in @('GetDpiForWindow', 'SetProcessDpiAwarenessContext') })
Write-Output "Mandatory recent DPI imports: $($modern.Count)"
foreach ($entry in $modern) { Write-Output "  $entry" }
if ($RejectModernDpiImports -and $modern.Count -ne 0) { throw 'Optional DPI APIs were linked as mandatory imports.' }

$context = [Rinva.StartupProbe]::Context($exe)
$cookie = [UIntPtr]::Zero
$active = $false
$missing = 0
$previousErrorMode = [Rinva.StartupProbe]::SetErrorMode(0x8001)
try {
    $active = [Rinva.StartupProbe]::ActivateActCtx($context, [ref]$cookie)
    if (-not $active) { throw 'Could not activate the executable manifest.' }
    foreach ($group in ($imports | Group-Object Dll)) {
        # Restrict resolution to Windows system DLLs and the manifest's SxS assemblies.
        # Do not load arbitrary DLLs from the project, current directory or PATH.
        $module = [Rinva.StartupProbe]::LoadLibraryExW($group.Name, [IntPtr]::Zero, 0x800)
        if ($module -eq [IntPtr]::Zero) {
            $missing++
            Write-Output "MISSING DLL: $($group.Name), Win32 error $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
            continue
        }
        try {
            Write-Output "DLL: $([Rinva.StartupProbe]::ModulePath($module))"
            foreach ($entry in $group.Group) {
                if (-not [Rinva.StartupProbe]::HasImport($module, $entry)) {
                    $missing++
                    Write-Output "MISSING ENTRY POINT: $entry"
                }
            }
        } finally { [void][Rinva.StartupProbe]::FreeLibrary($module) }
    }
} finally {
    if ($active) { [void][Rinva.StartupProbe]::DeactivateActCtx(0, $cookie) }
    [Rinva.StartupProbe]::ReleaseActCtx($context)
    [void][Rinva.StartupProbe]::SetErrorMode($previousErrorMode)
}
Write-Output "Checked $($imports.Count) imports. Missing system dependencies: $missing"
Write-Output 'This check does not execute Rinva or inspect DLL overrides in the IDE/project directory.'
if ($FailOnMissingImports -and $missing -ne 0) { throw "Missing system dependencies: $missing" }
