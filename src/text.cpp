#include "astro/text.hpp"
#include <cmath>

namespace astro {
void draw_moving_text(ImDrawList& draw, ImVec2 position, ImU32 color, const char* text) {
    const int first_vertex = draw.VtxBuffer.Size;
    draw.AddText(position, color, text);

    // ImFont::RenderText truncates the origin before emitting glyph quads.
    // Restore the common fractional translation without changing glyph spacing,
    // texture coordinates, draw commands, or unrelated geometry in this list.
    const ImVec2 fraction{position.x - std::trunc(position.x), position.y - std::trunc(position.y)};
    for (int i = first_vertex; i < draw.VtxBuffer.Size; ++i) {
        draw.VtxBuffer[i].pos.x += fraction.x;
        draw.VtxBuffer[i].pos.y += fraction.y;
    }
}
} // namespace astro
