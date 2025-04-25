/**
 * Taiga
 * Copyright (C) 2010-2025, Eren Okka
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

#include "recognition.hpp"

#include <algorithm>
#include <anitomy.hpp>
#include <ranges>
#include <vector>

#include "media/anime.hpp"
#include "track/episode.hpp"
#include "track/recognition_cache.hpp"
#include "track/recognition_normalize.hpp"

namespace track::recognition {

Episode parse(std::string_view input, const anitomy::Options options) {
  Episode episode;

  // @TODO: Separate filename from path

  auto elements = anitomy::parse(input, options);
  episode.setElements(elements);

  return episode;
}

int identify(Episode& episode) {
  cache()->init();

  const auto title = episode.element(anitomy::ElementKind::Title);
  const auto normalizedTitle = normalize(title);

  std::vector<Cache::Data::Match> matches;

  if (const auto data = cache()->find(normalizedTitle)) {
    // @TODO: validate matches
    matches.append_range(data->matches | std::views::values | std::ranges::to<std::vector>());
  }

  std::ranges::sort(matches, {}, &Cache::Data::Match::score);

  if (matches.empty()) return anime::kUnknownId;

  return matches.front().id;
}

}  // namespace track::recognition
