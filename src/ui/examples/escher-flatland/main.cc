// Copyright 2024 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <lib/async-loop/cpp/loop.h>
#include <lib/async-loop/default.h>
#include <lib/async/cpp/task.h>
#include <lib/async/default.h>
#include <lib/syslog/cpp/macros.h>
#include <lib/trace-provider/provider.h>

#include <chrono>
#include <ctime>
#include <sstream>

#include "src/ui/lib/escher/escher_flatland/escher_flatland.h"
#include "src/ui/lib/escher/renderer/frame.h"

// We might want to detect changes in the view size, and recreate the swapchain when it happens.
constexpr vk::Extent2D kExtent = vk::Extent2D(800, 600);

std::string GetCurrentTimeString(void) {
  // TODO(https://fxbug.dev/344700148): This clock is broken, and always returns 21:10:26.
  // In the interim, consider replacing the following code with native Zircon calls.
  std::time_t now = std::chrono::system_clock::to_time_t(std::chrono::system_clock::now());
  std::tm* time_struct = std::localtime(&now);
  std::ostringstream oss;
  oss << time_struct->tm_hour << ":" << time_struct->tm_min << ":" << time_struct->tm_sec;
  return oss.str();
}

int main(int argc, char** argv) {
  escher::EscherFlatland escher_flatland;
  async::Loop loop(&kAsyncLoopConfigAttachToCurrentThread);
  trace::TraceProviderWithFdio trace_provider(loop.dispatcher());

  escher::DebugFont debug_font = escher_flatland.debug_font();
  escher::RenderFrameFn render_frame = [&debug_font](const escher::ImagePtr& output_image,
                                                     const escher::FramePtr& frame) {
    static int32_t dir_x = 3;
    static int32_t dir_y = 1;
    static int32_t pos_x = 0;
    static int32_t pos_y = 0;

    auto time_string = GetCurrentTimeString();

    constexpr int32_t kScale = 4;
    const int32_t kWidth =
        static_cast<int32_t>(time_string.length()) * escher::DebugFont::kGlyphWidth * kScale;
    constexpr int32_t kHeight = escher::DebugFont::kGlyphHeight * kScale;

    if (dir_x < 0 && pos_x <= 0)
      dir_x *= -1;
    if (dir_y < 0 && pos_y <= 0)
      dir_y *= -1;
    if (dir_x > 0 && (pos_x + kWidth) >= static_cast<int32_t>(kExtent.width))
      dir_x *= -1;
    if (dir_y > 0 && (pos_y + kHeight) >= static_cast<int32_t>(kExtent.height))
      dir_y *= -1;

    pos_x += dir_x;
    pos_y += dir_y;

    debug_font.Blit(frame->cmds(), time_string.c_str(), output_image, {pos_x, pos_y}, kScale);
  };

  async::PostTask(loop.dispatcher(),
                  [&escher_flatland, &render_frame] { escher_flatland.RenderFrame(render_frame); });

  loop.Run();

  return 0;
}
