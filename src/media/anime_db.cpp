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

#include <QFile>
#include <format>

#include "base/string.hpp"
#include "compat/anime.hpp"
#include "compat/list.hpp"
#include "taiga/accounts.hpp"
#include "taiga/path.hpp"
#include "taiga/settings.hpp"
#include "taiga/version.hpp"

namespace {

std::string serviceUsername(const std::string& service) {
  if (service == "anilist") return taiga::accounts.anilistUsername();
  if (service == "kitsu") return taiga::accounts.kitsuUsername();
  if (service == "myanimelist") return taiga::accounts.myanimelistUsername();
  return std::string{};
}

}  // namespace

namespace anime {

void Database::init() {
  if (!QFile::exists(fileName())) {
    migrateItemsFromV1();
    migrateListEntriesFromV1();
    return;
  }
}

const Anime* Database::item(const int id) const {
  const auto it = items_.find(id);
  return it != items_.end() ? &(*it) : nullptr;
}

const ListEntry* Database::entry(const int id) const {
  const auto it = entries_.find(id);
  return it != entries_.end() ? &(*it) : nullptr;
}

const QMap<int, Anime>& Database::items() const {
  return items_;
}

const QMap<int, ListEntry>& Database::entries() const {
  return entries_;
}

QString Database::fileName() const {
  return u"%1/media.sqlite"_s.arg(QString::fromStdString(taiga::get_data_path()));
}

void Database::migrateItemsFromV1() {
  const auto path = std::format("{}/v1/db/anime.xml", taiga::get_data_path());

  auto items = compat::v1::readAnimeDatabase(path);

  for (auto&& item : items) {
    items_[item.id] = std::move(item);
  }
}

void Database::migrateListEntriesFromV1() {
  const auto service = taiga::settings.service();
  const auto username = serviceUsername(service);
  const auto path =
      std::format("{}/v1/user/{}@{}/anime.xml", taiga::get_data_path(), username, service);

  auto entries = compat::v1::readListEntries(path);

  for (auto&& entry : entries) {
    if (!items_.contains(entry.anime_id)) continue;
    entries_[entry.anime_id] = std::move(entry);
  }
}

}  // namespace anime
