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

#include "recognition_cache.hpp"

#include <format>

#include "media/anime_db.hpp"
#include "track/recognition.hpp"

namespace track::recognition {

bool Cache::empty() const {
  return titles_.empty();
}

const std::optional<Cache::Data> Cache::find(const std::string& title) const {
  const auto it = titles_.find(title);
  if (it == titles_.end()) return std::nullopt;
  return it->second;
}

void Cache::clear() {
  return titles_.clear();
}

void Cache::init() {
  if (!empty()) return;

  for (const auto& item : anime::db.items()) {
    add(item);
  }
}

void Cache::add(const anime::Details& item) {
  const auto add = [this, &item](const std::string& title, const float weight = 1.0f) {
    const auto normalized = normalize(title);
    if (normalized.empty()) return;
    titles_[normalized].matches.emplace(item.id, Data::Match{item.id, weight});
  };

  // @TODO: Add user-defined titles with higher weight

  // Main titles
  add(item.titles.romaji);
  add(item.titles.english);
  add(item.titles.japanese);

  // Main title + year
  if (item.date_started.year()) {
    const auto year = std::format("{}", item.date_started.year());
    if (!item.titles.romaji.contains(year)) {
      add(std::format("{} ({})", item.titles.romaji, year), 0.5f);
    }
  }

  // Synonyms
  for (const auto& synonym : item.titles.synonyms) {
    add(synonym, 0.5f);
  }
}

void Cache::remove(const anime::Details& item) {
  for (auto& [_, data] : titles_) {
    data.matches.erase(item.id);
  }
}

void Cache::update(const anime::Details& item) {
  remove(item);
  add(item);
}

}  // namespace track::recognition
