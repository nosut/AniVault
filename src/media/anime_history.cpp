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

#include "anime_history.hpp"

#include <format>

#include "compat/history.hpp"
#include "taiga/accounts.hpp"
#include "taiga/path.hpp"
#include "taiga/settings.hpp"

namespace anime {

void History::init() {
  const auto path = []() {
    const auto service = taiga::settings.service();
    return std::format("{}/v1/user/{}@{}/history.xml", taiga::get_data_path(),
                       taiga::accounts.serviceUsername(service), service);
  }();

  items_ = compat::v1::readHistory(path);
}

const QList<HistoryItem>& History::items() const {
  return items_;
}

}  // namespace anime
