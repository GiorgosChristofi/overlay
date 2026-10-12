param([string]$Title = "LoL Overlay items", [string]$Out = "overlay.png")
# Captures only the window with this exact title (not the whole screen).
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Win {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
Add-Type -AssemblyName System.Drawing
$found = $null
[Win]::EnumWindows({ param($h, $l)
  $sb = New-Object System.Text.StringBuilder 256
  [void][Win]::GetWindowText($h, $sb, 256)
  if ($sb.ToString() -eq $Title -and [Win]::IsWindowVisible($h)) { $script:found = $h; return $false }
  return $true }, [IntPtr]::Zero) | Out-Null
if (-not $found) { throw "window '$Title' not found" }
$r = New-Object Win+RECT
[void][Win]::GetWindowRect($found, [ref]$r)
$w = $r.R - $r.L; $h = $r.B - $r.T
"rect: $($r.L),$($r.T) ${w}x${h}"
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $h))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"saved $Out"
