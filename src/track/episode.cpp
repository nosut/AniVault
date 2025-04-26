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

#include "episode.hpp"

#include <ranges>

#include "media/anime.hpp"

namespace track {

Episode::Episode() : anime_id_{anime::kUnknownId} {}

auto Episode::find(const anitomy::ElementKind kind) const {
  const auto is_kind = [kind](const anitomy::Element& element) { return element.kind == kind; };
  return std::ranges::find_if(elements_, is_kind);
}

const std::vector<anitomy::Element>& Episode::elements() const noexcept {
  return elements_;
}

void Episode::setElements(std::vector<anitomy::Element>& elements) {
  elements_ = elements;
}

bool Episode::contains(const anitomy::ElementKind kind) const {
  return find(kind) != elements_.end();
}

std::string Episode::element(const anitomy::ElementKind kind) const {
  const auto it = find(kind);
  if (it != elements_.end()) return it->value;
  return {};
};

void Episode::addElement(const anitomy::ElementKind kind, const std::string& value) {
  elements_.emplace_back(anitomy::Element{.kind = kind, .value = value});
}

}  // namespace track
