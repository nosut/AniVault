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

#include <QXmlStreamReader>

#include "base/log.hpp"
#include "base/xml.hpp"
#include "taiga/accounts.hpp"
#include "taiga/settings.hpp"

#define XML_ATTR(name) xml.attributes().value(name)
#define XML_ATTR_STR(name) XML_ATTR(name).toString().toStdString()

namespace compat::v1 {

void parseAccountElement(QXmlStreamReader& xml, const taiga::Settings& settings,
                         const taiga::Accounts& accounts);
void parseAnimeElement(QXmlStreamReader& xml, const taiga::Settings& settings);

void readSettings(const std::string& path, const taiga::Settings& settings,
                  const taiga::Accounts& accounts) {
  base::XmlFileReader xml;

  if (!xml.open(QString::fromStdString(path))) {
    LOGE("{}", xml.file().errorString().toStdString());
    return;
  }

  if (!xml.readElement(u"settings")) {
    xml.raiseError("Invalid settings file.");
  }

  while (xml.readNextStartElement()) {
    if (xml.name() == u"account") {
      parseAccountElement(xml, settings, accounts);
    } else if (xml.name() == u"anime") {
      parseAnimeElement(xml, settings);
    } else {
      // @TODO: recognition, program, announce, rss
      xml.skipCurrentElement();
    }
  }

  if (xml.hasError()) {
    LOGE("{}", xml.errorString().toStdString());
  }
}

void parseAccountElement(QXmlStreamReader& xml, const taiga::Settings& settings,
                         const taiga::Accounts& accounts) {
  while (xml.readNextStartElement()) {
    if (xml.name() == u"update") {
      settings.setService(XML_ATTR_STR(u"activeservice"));
      xml.skipCurrentElement();

    } else if (xml.name() == u"anilist") {
      accounts.setAnilistUsername(XML_ATTR_STR(u"username"));
      accounts.setAnilistToken(XML_ATTR_STR(u"token"));
      xml.skipCurrentElement();

    } else if (xml.name() == u"kitsu") {
      accounts.setKitsuEmail(XML_ATTR_STR(u"email"));
      accounts.setKitsuUsername(XML_ATTR_STR(u"username"));
      accounts.setKitsuPassword(XML_ATTR_STR(u"password"));
      xml.skipCurrentElement();

    } else if (xml.name() == u"myanimelist") {
      accounts.setMyanimelistUsername(XML_ATTR_STR(u"username"));
      accounts.setMyanimelistAccessToken(XML_ATTR_STR(u"accesstoken"));
      accounts.setMyanimelistRefreshToken(XML_ATTR_STR(u"refreshtoken"));
      xml.skipCurrentElement();

    } else {
      xml.skipCurrentElement();
    }
  }
}

void parseAnimeElement(QXmlStreamReader& xml, const taiga::Settings& settings) {
  std::vector<std::string> libraryFolders;

  while (xml.readNextStartElement()) {
    if (xml.name() == u"folders") {
      while (xml.readNextStartElement()) {
        if (xml.name() == u"root") {
          libraryFolders.push_back(XML_ATTR_STR(u"folder"));
          xml.skipCurrentElement();
        } else {
          xml.skipCurrentElement();
        }
      }

    } else {
      xml.skipCurrentElement();
    }
  }

  settings.setLibraryFolders(libraryFolders);
}

}  // namespace compat::v1
