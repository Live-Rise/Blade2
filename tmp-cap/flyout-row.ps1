# probe row/col edges in a rect region + find light blobs (dot). ASCII only.
param(
    [Parameter(Mandatory)][string]$Path,
    [int]$RX = 900,
    [int]$RY = 1300,
    [int]$RW = 420,
    [int]$RH = 2,
    [int]$Thresh = 40
)
Add-Type -AssemblyName System.Drawing
$code = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
public static class RowProbe {
    public static string Run(string path, int rx, int ry, int rw, int rh, int thresh) {
        using (var bmp = new Bitmap(path)) {
            var rect = new Rectangle(0, 0, bmp.Width, bmp.Height);
            var d = bmp.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            int W = bmp.Width;
            var b = new byte[W * bmp.Height * 4];
            System.Runtime.InteropServices.Marshal.Copy(d.Scan0, b, 0, b.Length);
            bmp.UnlockBits(d);
            Func<int,string> hex = (i) => string.Format("#{0:X2}{1:X2}{2:X2}", b[i+2], b[i+1], b[i]);
            var sw = new System.Text.StringBuilder();
            for (int y = ry; y < ry + rh; y++) {
                sw.AppendLine("-- row y=" + y + " --");
                int pi = ((y * W + rx) * 4);
                int pr = b[pi+2], pg = b[pi+1], pb = b[pi];
                for (int x = rx + 1; x < rx + rw; x++) {
                    int i = (y * W + x) * 4;
                    int delta = Math.Abs(b[i+2] - pr) + Math.Abs(b[i+1] - pg) + Math.Abs(b[i] - pb);
                    if (delta > thresh) sw.AppendLine("x=" + x + " " + hex(i - 4) + "->" + hex(i));
                    pr = b[i+2]; pg = b[i+1]; pb = b[i];
                }
            }
            return sw.ToString();
        }
    }
}
'@
Add-Type -TypeDefinition $code -ReferencedAssemblies System.Drawing
Write-Host ([RowProbe]::Run($Path, $RX, $RY, $RW, $RH, $Thresh))
