# measure flyout geometry: card edges, text-row y ranges, color samples. ASCII only for PS5.1.
param(
    [Parameter(Mandatory)][string]$Path,
    [int]$X = 1000,
    [int]$Y0 = 1100,
    [int]$Y1 = 1550,
    [int]$XW = 450,
    [int]$TextMinR = 150
)
Add-Type -AssemblyName System.Drawing
$code = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
public static class FlyoutProbe {
    public static string Run(string path, int x, int y0, int y1, int xw, int minR) {
        using (var bmp = new Bitmap(path)) {
            var rect = new Rectangle(0, 0, bmp.Width, bmp.Height);
            var d = bmp.LockBits(rect, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            int W = bmp.Width, H = bmp.Height;
            var b = new byte[W * H * 4];
            System.Runtime.InteropServices.Marshal.Copy(d.Scan0, b, 0, b.Length);
            bmp.UnlockBits(d);
            Func<int,int,int> px = (xx, yy) => yy * W * 4 + xx * 4;
            Func<int,string> hex = (i) => string.Format("#{0:X2}{1:X2}{2:X2}", b[i+2], b[i+1], b[i]);
            var sw = new System.Text.StringBuilder();
            sw.AppendLine("== column x=" + x + " edges (y " + y0 + ".." + y1 + ") ==");
            int pr = b[px(x, y0) + 2], pg = b[px(x, y0) + 1], pb = b[px(x, y0)];
            for (int y = y0 + 1; y < y1; y++) {
                int i = px(x, y);
                int delta = Math.Abs(b[i+2] - pr) + Math.Abs(b[i+1] - pg) + Math.Abs(b[i] - pb);
                if (delta > 40) sw.AppendLine("y=" + y + " delta=" + delta + " " + hex(i - 4) + "->" + hex(i));
                pr = b[i+2]; pg = b[i+1]; pb = b[i];
            }
            sw.AppendLine("== text rows (x " + x + ".." + (x + xw) + ") ==");
            bool inRow = false; int rowStart = 0;
            for (int y = y0; y < y1; y++) {
                bool has = false;
                for (int xx = x; xx < x + xw; xx += 2) {
                    int i = px(xx, y);
                    if (b[i+2] > minR && b[i+1] > minR && b[i] > minR) { has = true; break; }
                }
                if (has && !inRow) { inRow = true; rowStart = y; }
                if (!has && inRow) {
                    inRow = false;
                    if (y - rowStart > 2) sw.AppendLine("row y " + rowStart + ".." + (y - 1) + " h=" + (y - rowStart));
                }
            }
            sw.AppendLine("== samples ==");
            int[][] pts = { new[]{x, y0}, new[]{x, y0+5}, new[]{x, y1-5}, new[]{x+200, y0+40} };
            foreach (var p in pts) sw.AppendLine("(" + p[0] + "," + p[1] + ") = " + hex(px(p[0], p[1])));
            return sw.ToString();
        }
    }
}
'@
Add-Type -TypeDefinition $code -ReferencedAssemblies System.Drawing
Write-Host ([FlyoutProbe]::Run($Path, $X, $Y0, $Y1, $XW, $TextMinR))
