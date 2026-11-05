# 列出 Blade2 各进程的所有顶层窗口（hwnd/类名/标题/矩形），用来挑真正的主窗口
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class EnumWnd {
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr p);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  delegate bool EnumProc(IntPtr h, IntPtr p);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  static List<string> found = new List<string>();
  static uint target = 0;

  public static List<string> Run(uint pid) {
    found = new List<string>();
    target = pid;
    EnumWindows(Cb, IntPtr.Zero);
    return found;
  }

  static bool Cb(IntPtr h, IntPtr p) {
    uint pid;
    GetWindowThreadProcessId(h, out pid);
    if (wpidMatch(h, pid)) {
      var cls = new StringBuilder(256); GetClassName(h, cls, 256);
      int len = GetWindowTextLength(h);
      var txt = new StringBuilder(len + 1); if (len > 0) GetWindowText(h, txt, len + 1);
      RECT r; GetWindowRect(h, out r);
      found.Add(string.Format("{0}|{1}|{2}|{3},{4} {5}x{6}", h.ToInt64(), cls, txt,
        r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top));
    }
    return true;
  }

  static bool wpidMatch(IntPtr h, uint pid) { return pid == target && IsWindowVisible(h); }
}
'@
foreach ($pr in (Get-Process -Name Blade2 -ErrorAction SilentlyContinue)) {
  Write-Host ("--- pid " + $pr.Id + " ---")
  foreach ($line in [EnumWnd]::Run([uint32]$pr.Id)) { Write-Host ("  " + $line) }
}
