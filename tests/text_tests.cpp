#include "astro/text.hpp"
#include <cmath>
#include <filesystem>
#include <iostream>
#include <stdexcept>
#include <vector>

namespace {
int checks = 0;

void require(bool condition, const char* message) {
    ++checks;
    if (!condition) {
        throw std::runtime_error(message);
    }
}

std::vector<ImDrawVert> vertices(const ImDrawList& draw, int first) {
    return {draw.VtxBuffer.Data + first, draw.VtxBuffer.Data + draw.VtxBuffer.Size};
}

void check_motion(const std::filesystem::path& font_path, float density, ImVec2 origin) {
    ImGui::CreateContext();
    auto& io = ImGui::GetIO();
    io.IniFilename = nullptr;
    io.DisplaySize = {800, 600};
    io.DisplayFramebufferScale = {density, density};
    io.FontGlobalScale = 1 / density;
    io.DeltaTime = 1.f / 120;

    ImFontGlyphRangesBuilder builder;
    builder.AddRanges(io.Fonts->GetGlyphRangesDefault());
    builder.AddText("月球天狼星");
    ImVector<ImWchar> ranges;
    builder.BuildRanges(&ranges);
    ImFontConfig config;
    config.OversampleH = config.OversampleV = 1;
    require(io.Fonts->AddFontFromFileTTF(
                font_path.string().c_str(), 15 * density, &config, ranges.Data),
            "load the production CJK font");
    require(io.Fonts->Build(), "build the font atlas");
    io.Fonts->SetTexID(ImTextureID(1));

    const char* text = "月球 Moon / 天狼星 Sirius 0123456789";
    std::vector<ImDrawVert> reference;
    ImVec2 previous_old{};
    int quantized_frames = 0;
    for (int frame = 0; frame < 65; ++frame) {
        const ImVec2 offset{frame * .125f, frame * -.0625f};
        const ImVec2 position{origin.x + offset.x, origin.y + offset.y};
        ImGui::NewFrame();
        auto& draw = *ImGui::GetBackgroundDrawList();
        draw.PushClipRect({-200, -200}, {1000, 800}, false);
        draw.AddRectFilled({20, 20}, {30, 30}, IM_COL32_WHITE);
        const auto unrelated = draw.VtxBuffer[0];

        int first = draw.VtxBuffer.Size;
        draw.AddText(position, IM_COL32_WHITE, text);
        const auto snapped = vertices(draw, first);
        require(!snapped.empty(), "ordinary text emits glyphs");
        if (frame && snapped[0].pos.x == previous_old.x) {
            ++quantized_frames;
        }
        previous_old = snapped[0].pos;

        first = draw.VtxBuffer.Size;
        astro::draw_moving_text(draw, position, IM_COL32_WHITE, text);
        const auto moving = vertices(draw, first);
        if (frame == 0) {
            reference = moving;
        }
        require(moving.size() == reference.size(),
                "moving label keeps all Chinese and Latin glyphs");
        for (size_t i = 0; i < moving.size(); ++i) {
            require(std::abs(moving[i].pos.x - reference[i].pos.x - offset.x) < 3e-5f &&
                        std::abs(moving[i].pos.y - reference[i].pos.y - offset.y) < 3e-5f,
                    "every glyph follows fractional motion without pixel steps");
            require(moving[i].uv.x == reference[i].uv.x && moving[i].uv.y == reference[i].uv.y,
                    "fractional motion preserves glyph sampling and spacing");
        }

        first = draw.VtxBuffer.Size;
        astro::draw_moving_text(
            draw, {position.x + 1, position.y + 1}, IM_COL32(0, 0, 0, 180), text);
        const auto shadow = vertices(draw, first);
        require(shadow.size() == moving.size(), "shadow and text contain the same glyphs");
        for (size_t i = 0; i < shadow.size(); ++i) {
            require(std::abs(shadow[i].pos.x - moving[i].pos.x - 1) < 3e-5f &&
                        std::abs(shadow[i].pos.y - moving[i].pos.y - 1) < 3e-5f,
                    "shadow offset remains constant while crossing pixel boundaries");
        }
        require(draw.VtxBuffer[0].pos.x == unrelated.pos.x &&
                    draw.VtxBuffer[0].pos.y == unrelated.pos.y,
                "text correction leaves existing UI geometry unchanged");
        const int count = draw.VtxBuffer.Size;
        astro::draw_moving_text(draw, position, 0, text);
        astro::draw_moving_text(draw, position, IM_COL32_WHITE, "");
        require(draw.VtxBuffer.Size == count, "empty and transparent labels add no geometry");
        draw.PopClipRect();
        ImGui::Render();
    }
    require(quantized_frames > 40,
            "regression reproduces integer snapping in the original text path");
    std::cout << density << "x density: original path held its x coordinate on " << quantized_frames
              << "/64 frame transitions; corrected path followed all 64 fractional steps\n";
    ImGui::DestroyContext();
}
} // namespace

int main(int argc, char** argv) {
    try {
        const auto data = std::filesystem::path(argc > 1 ? argv[1] : "data");
        for (float density : {1.f, 2.f}) {
            check_motion(data / "fonts/NotoSansCJKsc-Regular.otf", density, {100, 100});
            check_motion(data / "fonts/NotoSansCJKsc-Regular.otf", density, {-.25f, -.75f});
        }
        std::cout << "Passed " << checks << " text-motion checks\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "FAIL: " << error.what() << '\n';
        if (ImGui::GetCurrentContext()) {
            ImGui::DestroyContext();
        }
        return 1;
    }
}
