# find accent-colored circle bbox in a region (send button calibration). ASCII only.
param(
    [Parameter(Mandatory)][string]$Path,
    [int]$RX = 2400,
    [int]$RY = 1480,
    [int]$RW = 160,
    [int]$RH = 120
)
Add-Type -AssemblyName System.Drawing
$code = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
public static class BboxProbe {
    public static string Run(string path, int rx, int ry, int rw, int rh) {
        using (var bmp = new Bitmap(path)) {
            var rect = new Rectangle(0, 0, bmp.Width, bmp.Height);
            var d = bmp.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            int W = bmp.Width;
            var b = new byte[W * bmp.Height * 4];
            System.Runtime.InteropServices.Marshal.Copy(d.Scan0, b, 0, b.Length);
            bmp.UnlockBits(d);
            int minX = int.MaxValue, minY = int.MaxValue, maxX = -1, maxY = -1;
            string sample = "";
            for (int y = ry; y < ry + rh; y++) {
                for (int x = rx; x < rx + rw; x++) {
                    int i = (y * W + x) * 4;
                    int R = b[i + 2], G = b[i + 1], B = b[i];
                    // accent pink: strong R, low-mid G/B
                    if (R > 180 && G < 140 && B < 160 && R - G > 70) {
                        if (x < minX) minX = x; if (x > maxX) maxX = x;
                        if (y < minY) minY = y; if (y > maxY) maxY = y;
                        if (sample.Length == 0) sample = string.Format("#{0:X2}{1:X2}{2:X2}", R, G, B) + " at " + x + "," + y;
                    }
                }
            }
            return "bbox x " + minX + ".." + maxX + " (w=" + (maxX - minX + 1) + ")  y " + minY + ".." + maxY + " (h=" + (maxY - minY + 1) + ")  sample " + sample;
        }
    }
}
'@
Add-Type -TypeDefinition $code -ReferencedAssemblies System.Drawing
Write-Host ([BboxProbe]::Run($Path, $RX, $RY, $RW, $RH))
