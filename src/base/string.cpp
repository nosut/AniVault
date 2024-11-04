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

#include "string.hpp"

#include <QRegularExpression>
#include <nstd/string.hpp>
#include <ranges>

namespace {

[[nodiscard]] std::string toStdString(const QString& s) {
  return s.toStdString();
}

}  // namespace

QString joinStrings(const std::vector<std::string>& list, QString placeholder) {
  if (list.empty()) return placeholder;
  return QString::fromStdString(nstd::join(list, ", "));
}

void removeHtmlTags(QString& str) {
  static const QRegularExpression re{"<[a-z/]+>"};
  str.remove(re);
}

std::vector<std::string> toVector(const QStringList& list) {
  return list | std::views::transform(toStdString) | std::ranges::to<std::vector>();
}
