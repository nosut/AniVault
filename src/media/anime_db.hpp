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

#pragma once

#include <QMap>
#include <QSqlDatabase>

#include "media/anime.hpp"
#include "media/anime_list.hpp"

namespace anime {

class Database final : public QObject {
  Q_OBJECT
  Q_DISABLE_COPY_MOVE(Database)

public:
  Database();
  ~Database() = default;

  void init();

  const Anime* item(const int id) const;
  const ListEntry* entry(const int id) const;

  const QMap<int, Anime>& items() const;
  const QMap<int, ListEntry>& entries() const;

private:
  QString fileName() const;
  QString sql(const QString& name) const;

  void createTables();
  QString currentVersion();

  void readItems();
  void readEntries();

  void migrateItemsFromV1();
  void migrateListEntriesFromV1();

  QSqlDatabase db_;

  QMap<int, Anime> items_;
  QMap<int, ListEntry> entries_;
};

inline Database db;

}  // namespace anime
