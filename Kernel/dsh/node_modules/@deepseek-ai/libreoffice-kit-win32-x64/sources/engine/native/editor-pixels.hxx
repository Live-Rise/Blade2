// Normalize LibreOfficeKit's premultiplied tiles in their existing allocation.
#pragma once
#include <algorithm>
#include <stdexcept>
#include <vector>

namespace dsh::editor {
// Match ImageData's Uint8ClampedArray rounding, including half-to-even ties.
inline unsigned char clampedChannel(double value)
{
    if (value >= 255) return 255;
    const auto floor = static_cast<unsigned int>(value);
    const double fraction = value - floor;
    return static_cast<unsigned char>(floor + (fraction > 0.5 || (fraction == 0.5 && (floor & 1))));
}

inline void straightRgba(std::vector<unsigned char>& pixels, int tileMode)
{
    if ((tileMode != 0 && tileMode != 1) || pixels.size() % 4 != 0)
        throw std::runtime_error("Unsupported native editor tile format");
    for (size_t offset = 0; offset < pixels.size(); offset += 4) {
        auto* pixel = pixels.data() + offset;
        const auto alpha = pixel[3];
        if (alpha == 0) pixel[0] = pixel[1] = pixel[2] = 0;
        else {
            if (tileMode == 1) std::swap(pixel[0], pixel[2]);
            if (alpha != 255) {
                const double multiplier = 255.0 / alpha;
                pixel[0] = clampedChannel(pixel[0] * multiplier);
                pixel[1] = clampedChannel(pixel[1] * multiplier);
                pixel[2] = clampedChannel(pixel[2] * multiplier);
            }
        }
    }
}
}
