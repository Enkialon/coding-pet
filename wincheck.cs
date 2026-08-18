using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

class Program {
    [DllImport("user32.dll")]
    static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")]
    static extern int GetWindowLong(IntPtr hWnd, int nIndex);
    [DllImport("user32.dll")]
    static extern bool IsWindowVisible(IntPtr hWnd);
    delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")]
    static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);
    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    static extern int GetClassName(IntPtr hWnd, StringBuilder lpClassName, int nMaxCount);
    
    [StructLayout(LayoutKind.Sequential)]
    struct RECT { public int Left, Top, Right, Bottom; }
    
    static void Main() {
        uint targetPid = 11976;
        EnumWindows((hWnd, lParam) => {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid == targetPid) {
                RECT rect;
                GetWindowRect(hWnd, out rect);
                int exStyle = GetWindowLong(hWnd, -20);
                bool visible = IsWindowVisible(hWnd);
                int w = rect.Right - rect.Left;
                int h = rect.Bottom - rect.Top;
                var sb = new StringBuilder(256);
                GetClassName(hWnd, sb, 256);
                Console.WriteLine($"HWND=0x{hWnd.ToInt64():X} Class={sb} Visible={visible} Size={w}x{h} Pos=({rect.Left},{rect.Top}) ExStyle=0x{exStyle:X8}");
            }
            return true;
        }, IntPtr.Zero);
    }
}
