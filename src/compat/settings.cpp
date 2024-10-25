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
#include <QXmlStreamReader>

#include "taiga/settings.hpp"

namespace compat::v1 {

void readSettings(const std::string& path, const taiga::Settings& settings) {
  QFile file(QString::fromStdString(path));

  if (!file.open(QIODevice::ReadOnly)) return;

  QXmlStreamReader xml(&file);

  if (!xml.readNextStartElement()) return;
  if (xml.name() != u"settings") return;

  std::vector<std::string> libraryFolders;

  while (xml.readNextStartElement()) {
    if (xml.name() == u"account") {
      while (xml.readNextStartElement()) {
        if (xml.name() == u"update") {
          settings.setService(xml.attributes().value(u"activeservice").toString().toStdString());
        } else if (xml.name() == QString::fromStdString(settings.service())) {
          settings.setUsername(xml.attributes().value(u"username").toString().toStdString());
        }
        xml.skipCurrentElement();
      }
    } else if (xml.name() == u"anime") {
      while (xml.readNextStartElement()) {
        if (xml.name() == u"folders") {
          while (xml.readNextStartElement()) {
            if (xml.name() == u"root") {
              libraryFolders.push_back(xml.attributes().value(u"folder").toString().toStdString());
            }
            xml.skipCurrentElement();
          }
        }
        xml.skipCurrentElement();
      }
    } else {
      xml.skipCurrentElement();
    }
  }

  settings.setLibraryFolders(libraryFolders);
}

}  // namespace compat::v1
