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

#include "session.hpp"

#include <QByteArray>

#include "base/string.hpp"
#include "gui/list/list_widget.hpp"
#include "gui/models/anime_list_model.hpp"
#include "taiga/path.hpp"

namespace taiga {

QString Session::fileName() const {
  return u"%1/session.json"_s.arg(QString::fromStdString(get_data_path()));
}

////////////////////////////////////////////////////////////////////////////////

int Session::animeListSortColumn() const {
  return value("animeList.sortColumn", gui::AnimeListModel::COLUMN_TITLE).toInt();
}

Qt::SortOrder Session::animeListSortOrder() const {
  return value("animeList.sortOrder", Qt::SortOrder::AscendingOrder).value<Qt::SortOrder>();
}

gui::ListViewMode Session::animeListViewMode() const {
  return value("animeList.viewMode", static_cast<int>(gui::ListViewMode::List))
      .value<gui::ListViewMode>();
}

QByteArray Session::mainWindowGeometry() const {
  return QByteArray::fromBase64(value("mainWindow.geometry", QByteArray{}).toByteArray());
}

QByteArray Session::mediaDialogGeometry() const {
  return QByteArray::fromBase64(value("mediaDialog.geometry", QByteArray{}).toByteArray());
}

QByteArray Session::mediaDialogSplitterState() const {
  return QByteArray::fromBase64(value("mediaDialog.splitterState", QByteArray{}).toByteArray());
}

////////////////////////////////////////////////////////////////////////////////

void Session::setAnimeListSortColumn(const int column) const {
  setValue("animeList.sortColumn", column);
}

void Session::setAnimeListSortOrder(const Qt::SortOrder order) const {
  setValue("animeList.sortOrder", order);
}

void Session::setAnimeListViewMode(const gui::ListViewMode mode) const {
  setValue("animeList.viewMode", static_cast<int>(mode));
}

void Session::setMainWindowGeometry(const QByteArray& geometry) const {
  setValue("mainWindow.geometry", geometry.toBase64());
}

void Session::setMediaDialogGeometry(const QByteArray& geometry) const {
  setValue("mediaDialog.geometry", geometry.toBase64());
}

void Session::setMediaDialogSplitterState(const QByteArray& state) const {
  setValue("mediaDialog.splitterState", state.toBase64());
}

}  // namespace taiga
