cmake_minimum_required(VERSION 3.24)
project(
    Astra
    VERSION 0.1.0
    LANGUAGES C CXX
)
if(MSVC)
    add_compile_options(/utf-8)
endif()
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
if(APPLE)
    list(APPEND CMAKE_PREFIX_PATH /opt/homebrew /usr/local)
endif()
option(ASTRA_BUILD_APP "Build the Vulkan desktop application" ON)
find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
    pkg_check_modules(ERFA QUIET IMPORTED_TARGET erfa)
endif()
if(TARGET PkgConfig::ERFA)
    add_library(astra_erfa ALIAS PkgConfig::ERFA)
else()
    find_path(ERFA_INCLUDE_DIR erfa.h REQUIRED)
    find_library(ERFA_LIBRARY NAMES erfa REQUIRED)
    add_library(astra_erfa UNKNOWN IMPORTED)
    set_target_properties(
        astra_erfa PROPERTIES IMPORTED_LOCATION "${ERFA_LIBRARY}" INTERFACE_INCLUDE_DIRECTORIES
                                                                  "${ERFA_INCLUDE_DIR}"
    )
endif()
find_package(Threads REQUIRED)
set(CSPICE_ROOT
    "${CMAKE_SOURCE_DIR}/third_party/cspice"
    CACHE PATH "NAIF CSPICE distribution"
)
find_path(
    CSPICE_INCLUDE SpiceUsr.h
    PATHS "${CSPICE_ROOT}/include" REQUIRED
    NO_DEFAULT_PATH
)
find_library(
    CSPICE_LIBRARY
    NAMES cspice cspice.a
    PATHS "${CSPICE_ROOT}/lib" REQUIRED
    NO_DEFAULT_PATH
)
set(IMGUI_ROOT "${CMAKE_SOURCE_DIR}/third_party/imgui-1.91.9b")
if(NOT EXISTS "${IMGUI_ROOT}/imgui.cpp")
    message(FATAL_ERROR "Run python3 scripts/bootstrap.py --dependencies first")
endif()
add_library(
    astro_core
    src/time.cpp
    src/orientation.cpp
    src/catalog.cpp
    src/sky.cpp
    src/scenario.cpp
    src/camera.cpp
    src/background.cpp
    src/image_mips.cpp
    src/photometry.cpp
    src/events.cpp
    src/eclipse.cpp
    src/horizon.cpp
    src/render_scene.cpp
    src/i18n.cpp
)
target_include_directories(
    astro_core
    PUBLIC include third_party
    PRIVATE "${CSPICE_INCLUDE}"
)
target_link_libraries(
    astro_core
    PUBLIC astra_erfa Threads::Threads
    PRIVATE "${CSPICE_LIBRARY}"
)
target_compile_definitions(astro_core PUBLIC ASTRA_SOURCE_DATA="${CMAKE_SOURCE_DIR}/data")
if(MSVC)
    target_compile_options(astro_core PRIVATE /W4 /utf-8)
else()
    target_compile_options(astro_core PRIVATE -Wall -Wextra -Wpedantic)
endif()
if(ASTRA_BUILD_APP)
    find_package(Vulkan REQUIRED)
    find_package(SDL3 CONFIG REQUIRED)
    find_package(PNG REQUIRED)
    set(ATMOSPHERE_SOURCES
        shaders/atmosphere_filter.glsl shaders/atmosphere_model.glsl
        shaders/atmosphere_spectrum.glsl shaders/atmosphere.comp vendor/bruneton/definitions.glsl
        vendor/bruneton/functions.glsl
    )
    set_property(
        DIRECTORY
        APPEND
        PROPERTY CMAKE_CONFIGURE_DEPENDS ${ATMOSPHERE_SOURCES}
    )
    set(ATMOSPHERE_HASH_INPUT "")
    foreach(source IN LISTS ATMOSPHERE_SOURCES)
        file(SHA256 "${CMAKE_SOURCE_DIR}/${source}" digest)
        string(APPEND ATMOSPHERE_HASH_INPUT "${digest}")
    endforeach()
    string(SHA256 ATMOSPHERE_HASH "${ATMOSPHERE_HASH_INPUT}")
    file(READ "${CMAKE_SOURCE_DIR}/shaders/atmosphere_spectrum.glsl" spectrum)
    string(REGEX MATCH "SPECTRAL_GROUPS = ([0-9]+)" unused "${spectrum}")
    set(SPECTRAL_GROUPS "${CMAKE_MATCH_1}")
    add_library(astra_atmosphere_gpu src/atmosphere.cpp)
    target_include_directories(astra_atmosphere_gpu PUBLIC include)
    target_link_libraries(astra_atmosphere_gpu PUBLIC Vulkan::Vulkan)
    target_compile_definitions(
        astra_atmosphere_gpu PRIVATE ASTRA_ATMOSPHERE_ID="${ATMOSPHERE_HASH}"
                                     ASTRA_SPECTRAL_GROUPS=${SPECTRAL_GROUPS}
    )
    add_executable(astra_atmosphere src/atmosphere_cli.cpp)
    target_link_libraries(astra_atmosphere PRIVATE astra_atmosphere_gpu)
    target_compile_definitions(
        astra_atmosphere PRIVATE ASTRA_SHADER_DIR="${CMAKE_BINARY_DIR}/shaders"
    )
    add_library(
        imgui STATIC
        "${IMGUI_ROOT}/imgui.cpp" "${IMGUI_ROOT}/imgui_draw.cpp" "${IMGUI_ROOT}/imgui_tables.cpp"
        "${IMGUI_ROOT}/imgui_widgets.cpp" "${IMGUI_ROOT}/backends/imgui_impl_sdl3.cpp"
        "${IMGUI_ROOT}/backends/imgui_impl_vulkan.cpp"
    )
    target_include_directories(imgui PUBLIC "${IMGUI_ROOT}" "${IMGUI_ROOT}/backends")
    target_link_libraries(imgui PUBLIC SDL3::SDL3 Vulkan::Vulkan)
    add_executable(
        astra MACOSX_BUNDLE
        src/main.cpp
        src/app.cpp
        src/ui.cpp
        src/explore.cpp
        src/sequence.cpp
        src/recording.cpp
        src/text.cpp
        src/renderer.cpp
    )
    target_link_libraries(astra PRIVATE astro_core imgui PNG::PNG astra_atmosphere_gpu)
    target_compile_definitions(astra PRIVATE ASTRA_SHADER_DIR="${CMAKE_BINARY_DIR}/shaders")
    set_target_properties(
        astra
        PROPERTIES MACOSX_BUNDLE_BUNDLE_NAME "Astra"
                   MACOSX_BUNDLE_GUI_IDENTIFIER "local.astra.sky"
                   BUILD_RPATH "/opt/homebrew/lib;/usr/local/lib"
    )
    if(APPLE)
        set(ASTRA_APP_ICON "${CMAKE_SOURCE_DIR}/assets/macos/Astra.icns")
        target_sources(astra PRIVATE "${ASTRA_APP_ICON}")
        set_source_files_properties(
            "${ASTRA_APP_ICON}" PROPERTIES MACOSX_PACKAGE_LOCATION Resources
        )
        set_target_properties(astra PROPERTIES MACOSX_BUNDLE_ICON_FILE "Astra.icns")
        foreach(language en zh-Hans)
            set(LOCALIZED_INFO
                "${CMAKE_SOURCE_DIR}/assets/macos/${language}.lproj/InfoPlist.strings"
            )
            target_sources(astra PRIVATE "${LOCALIZED_INFO}")
            set_source_files_properties(
                "${LOCALIZED_INFO}" PROPERTIES MACOSX_PACKAGE_LOCATION
                                               "Resources/${language}.lproj"
            )
        endforeach()
    endif()
    find_program(GLSLANG_VALIDATOR glslangValidator REQUIRED)
    foreach(
        shader
        sky.vert
        sky.frag
        stars.vert
        stars.frag
        tonemap.frag
        atmosphere.comp
        exposure.comp
    )
        add_custom_command(
            OUTPUT "${CMAKE_BINARY_DIR}/shaders/${shader}.spv"
            COMMAND ${CMAKE_COMMAND} -E make_directory "${CMAKE_BINARY_DIR}/shaders"
            COMMAND
                "${GLSLANG_VALIDATOR}" -V --target-env vulkan1.2
                "${CMAKE_SOURCE_DIR}/shaders/${shader}" -o
                "${CMAKE_BINARY_DIR}/shaders/${shader}.spv"
            DEPENDS "${CMAKE_SOURCE_DIR}/shaders/features.glsl"
                    "${CMAKE_SOURCE_DIR}/shaders/atmosphere_render.glsl"
                    "${CMAKE_SOURCE_DIR}/shaders/${shader}"
                    "${CMAKE_SOURCE_DIR}/shaders/scene.glsl"
                    ${ATMOSPHERE_SOURCES}
            VERBATIM
        )
        list(APPEND SHADER_OUTPUTS "${CMAKE_BINARY_DIR}/shaders/${shader}.spv")
    endforeach()
    add_custom_target(astra_shaders DEPENDS ${SHADER_OUTPUTS})
    add_dependencies(astra astra_shaders)
    add_dependencies(astra_atmosphere astra_shaders)
endif()
add_executable(astra_cli src/cli.cpp)
target_link_libraries(astra_cli PRIVATE astro_core)
enable_testing()
add_executable(astro_tests tests/core_tests.cpp)
target_include_directories(astro_tests PRIVATE "${CSPICE_INCLUDE}")
target_link_libraries(astro_tests PRIVATE astro_core "${CSPICE_LIBRARY}")
add_test(NAME core COMMAND astro_tests "${CMAKE_SOURCE_DIR}/data")
add_executable(exploration_tests tests/exploration_tests.cpp)
target_link_libraries(exploration_tests PRIVATE astro_core)
add_test(NAME exploration COMMAND exploration_tests "${CMAKE_SOURCE_DIR}/data")
add_executable(camera_tests tests/camera_tests.cpp)
target_link_libraries(camera_tests PRIVATE astro_core)
add_test(NAME camera_drag COMMAND camera_tests)
add_executable(image_mips_tests tests/image_mips_tests.cpp)
target_link_libraries(image_mips_tests PRIVATE astro_core)
add_test(NAME image_mips COMMAND image_mips_tests)
add_executable(photometry_tests tests/photometry_tests.cpp)
target_link_libraries(photometry_tests PRIVATE astro_core)
add_test(NAME photometry COMMAND photometry_tests)
add_executable(i18n_tests tests/i18n_tests.cpp)
target_link_libraries(i18n_tests PRIVATE astro_core)
add_test(NAME i18n COMMAND i18n_tests)
if(ASTRA_BUILD_APP)
    add_executable(sequence_tests tests/sequence_tests.cpp src/sequence.cpp src/recording.cpp)
    target_link_libraries(sequence_tests PRIVATE astro_core PNG::PNG SDL3::SDL3)
    add_test(NAME sequence_export COMMAND sequence_tests)
    add_executable(text_tests tests/text_tests.cpp src/text.cpp)
    target_include_directories(text_tests PRIVATE include)
    target_link_libraries(text_tests PRIVATE imgui)
    add_test(NAME text_motion COMMAND text_tests "${CMAKE_SOURCE_DIR}/data")
endif()
