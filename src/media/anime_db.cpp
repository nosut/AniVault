/**
 * Taiga
 * Copyright (C) 2010-2024, Eren Okka
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

#include "anime_db.hpp"

#include <format>

#include "compat/anime.hpp"
#include "compat/list.hpp"
#include "taiga/accounts.hpp"
#include "taiga/path.hpp"
#include "taiga/settings.hpp"

namespace anime {

QList<Anime> readDatabase() {
  const auto data_path = taiga::get_data_path();
  return compat::v1::readAnimeDatabase(std::format("{}/v1/db/anime.xml", data_path));
}

QList<ListEntry> readListEntries() {
  const auto data_path = taiga::get_data_path();

  const auto service = taiga::settings.service();
  const auto username = [service]() {
    if (service == "anilist") return taiga::accounts.anilistUsername();
    if (service == "kitsu") return taiga::accounts.kitsuUsername();
    if (service == "myanimelist") return taiga::accounts.myanimelistUsername();
    return std::string{};
  }();

  return compat::v1::readListEntries(
      std::format("{}/v1/user/{}@{}/anime.xml", data_path, username, service));
}

}  // namespace anime
