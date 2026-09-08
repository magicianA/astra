#pragma once

#include <imgui.h>

namespace astro {
// Moving sky annotations must retain fractional coordinates. Ordinary ImGui
// text snaps its origin to a logical pixel, which becomes two pixels on Retina.
void draw_moving_text(ImDrawList&, ImVec2 position, ImU32 color, const char* text);
} // namespace astro
