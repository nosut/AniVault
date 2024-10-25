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

#include "taiga/path.hpp"

namespace taiga {

QString Session::fileName() const {
  return u"%1/session.json"_qs.arg(QString::fromStdString(get_data_path()));
}

////////////////////////////////////////////////////////////////////////////////

QByteArray Session::mainWindowGeometry() const {
  const auto geometry = value("mainWindow.geometry", QByteArray{}).toByteArray();
  return QByteArray::fromBase64(geometry);
}

////////////////////////////////////////////////////////////////////////////////

void Session::setMainWindowGeometry(const QByteArray& geometry) const {
  setValue("mainWindow.geometry", geometry.toBase64());
}

}  // namespace taiga
