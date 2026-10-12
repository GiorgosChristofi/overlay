param([int]$X, [int]$Y)
# Left click at screen coordinates (test VM only; never used against the game).
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Mouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
}
"@
[void][Mouse]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 100
[Mouse]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 60
[Mouse]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)
"clicked $X,$Y"
