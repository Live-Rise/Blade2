# report the brightest and dimmest pixel in a small rect (ink vs bg). ASCII only.
param(
    [Parameter(Mandatory)][string]$Path,
    [int]$RX, [int]$RY, [int]$RW, [int]$RH
)
Add-Type -AssemblyName System.Drawing
$code = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
public static class MaxProbe {
    public static string Run(string path, int rx, int ry, int rw, int rh) {
        using (var bmp = new Bitmap(path)) {
            var rect = new Rectangle(0, 0, bmp.Width, bmp.Height);
            var d = bmp.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            int W = bmp.Width;
            var b = new byte[W * bmp.Height * 4];
            System.Runtime.InteropServices.Marshal.Copy(d.Scan0, b, 0, b.Length);
            bmp.UnlockBits(d);
            int maxL = -1, minL = 100000; string maxHex = "", minHex = ""; int mx = 0, my = 0;
            for (int y = ry; y < ry + rh; y++) {
                for (int x = rx; x < rx + rw; x++) {
                    int i = (y * W + x) * 4;
                    int l = b[i] + b[i+1] + b[i+2];
                    string hex = string.Format("#{0:X2}{1:X2}{2:X2}", b[i+2], b[i+1], b[i]);
                    if (l > maxL) { maxL = l; maxHex = hex; mx = x; my = y; }
                    if (l < minL) { minL = l; minHex = hex; }
                }
            }
            return "brightest " + maxHex + " at " + mx + "," + my + "   dimmest " + minHex;
        }
    }
}
'@
Add-Type -TypeDefinition $code -ReferencedAssemblies System.Drawing
Write-Host ([MaxProbe]::Run($Path, $RX, $RY, $RW, $RH))
