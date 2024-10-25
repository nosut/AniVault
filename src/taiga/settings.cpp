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

#include "settings.hpp"

#include <QFile>
#include <QJsonArray>
#include <QSettings>
#include <ranges>

#include "compat/settings.hpp"
#include "taiga/path.hpp"

namespace taiga {

void Settings::migrate() const {
  if (!QFile::exists(fileName())) {
    compat::v1::readSettings(std::format("{}/v1/settings.xml", get_data_path()), *this);
  }
}

QString Settings::fileName() const {
  return u"%1/settings.ini"_qs.arg(QString::fromStdString(get_data_path()));
}

QSettings Settings::settings() const {
  return QSettings(fileName(), QSettings::IniFormat);
}

////////////////////////////////////////////////////////////////////////////////

std::string Settings::service() const {
  return settings().value("v1/service").toString().toStdString();
}

std::string Settings::username() const {
  return settings().value("v1/username").toString().toStdString();
}

std::vector<std::string> Settings::libraryFolders() const {
  return settings().value("library/folders").toJsonArray().toVariantList() |
         std::views::transform([](const QVariant& v) { return v.toString().toStdString(); }) |
         std::ranges::to<std::vector>();
}

////////////////////////////////////////////////////////////////////////////////

void Settings::setService(const std::string& service) const {
  settings().setValue("v1/service", QString::fromStdString(service));
}

void Settings::setUsername(const std::string& username) const {
  settings().setValue("v1/username", QString::fromStdString(username));
}

void Settings::setLibraryFolders(std::vector<std::string> folders) const {
  const auto list =
      folders |
      std::views::transform([](const std::string& s) { return QString::fromStdString(s); }) |
      std::ranges::to<QList>();
  settings().setValue("library/folders", QJsonArray::fromStringList(list));
}

}  // namespace taiga
