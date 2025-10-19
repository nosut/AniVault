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

#include "history.hpp"

#include "base/log.hpp"
#include "base/xml.hpp"
#include "compat/common.hpp"
#include "media/anime_history.hpp"

#define XML_ATTR(name) xml.attributes().value(name)
#define XML_ATTR_INT(name) XML_ATTR(name).toInt()
#define XML_ATTR_STR(name) XML_ATTR(name).toString().toStdString()

namespace compat::v1 {

void parseItems(QXmlStreamReader& xml, QList<anime::HistoryItem>& items);
void parseQueue(QXmlStreamReader& xml);

QList<anime::HistoryItem> readHistory(const std::string& path) {
  base::XmlFileReader xml;

  if (!xml.open(QString::fromStdString(path), removeMetaElement)) {
    LOGE("{}", xml.file().errorString().toStdString());
    return {};
  }

  if (!xml.readElement(u"history")) {
    xml.raiseError("Invalid history file.");
  }

  QList<anime::HistoryItem> items;

  while (xml.readNextStartElement()) {
    if (xml.name() == u"items") {
      parseItems(xml, items);
    } else if (xml.name() == u"queue") {
      parseQueue(xml);
    } else {
      xml.skipCurrentElement();
    }
  }

  if (xml.hasError()) {
    LOGE("{}", xml.errorString().toStdString());
    return {};
  }

  return items;
}

void parseItems(QXmlStreamReader& xml, QList<anime::HistoryItem>& items) {
  while (xml.readNextStartElement()) {
    if (xml.name() == u"item") {
      items.emplace_back(anime::HistoryItem{
        .anime_id = XML_ATTR_INT(u"anime_id"),
        .episode = XML_ATTR_INT(u"episode"),
        .time = XML_ATTR_STR(u"time"),
      });
      xml.skipCurrentElement();

    } else {
      xml.skipCurrentElement();
    }
  }
}

void parseQueue(QXmlStreamReader& xml) {
  // @TODO
  xml.skipCurrentElement();
}

}  // namespace compat::v1
