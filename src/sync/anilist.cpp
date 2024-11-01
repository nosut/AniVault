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

#include "anilist.hpp"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkProxy>
#include <QRestReply>

#include "base/file.hpp"
#include "base/log.hpp"
#include "base/string.hpp"
#include "media/anime_db.hpp"
#include "sync/anilist_parsers.hpp"
#include "sync/anilist_utils.hpp"
#include "taiga/accounts.hpp"
#include "taiga/config.h"

// AniList API documentation:
// https://docs.anilist.co/

namespace sync::anilist {

Service::Service() : QObject{}, manager_{new QNetworkAccessManager{}} {
  manager_.networkAccessManager()->setAutoDeleteReplies(true);
  manager_.networkAccessManager()->setTransferTimeout(std::chrono::seconds{10});

  connect(manager_.networkAccessManager(), &QNetworkAccessManager::finished, this,
          [](QNetworkReply* reply) {
            //
          });

  api_.setBaseUrl(QUrl{"https://graphql.anilist.co"});

  static const auto userAgentString = []() {
    return u"%1/%2.%3"_s.arg(TAIGA_APP_NAME).arg(TAIGA_VERSION_MAJOR).arg(TAIGA_VERSION_MINOR);
  };

  QHttpHeaders headers;
  headers.append(QHttpHeaders::WellKnownHeader::UserAgent, userAgentString());
  api_.setCommonHeaders(headers);

  if (const auto token = taiga::accounts.anilistToken(); !token.empty()) {
    api_.setBearerToken(QByteArray::fromStdString(token));
  }
}

void Service::fetchAnime(const int id) {
  const QJsonDocument data{{
      {"query", gql("Media")},
      {"variables", QJsonObject{{"id", id}}},
  }};

  const auto callback = [this](QRestReply& reply) {
    if (!reply.isSuccess()) {
      handleError(reply);
      return;
    }
    if (const auto json = reply.readJson()) {
      if (auto item = parseMedia((*json)["data"]["Media"])) {
        anime::db.updateItem(*item);
      } else {
        LOGE("Could not parse media object: {}", json->toJson().toStdString());
      }
    }
  };

  manager_.post(api_.createRequest(), data, this, callback);
}

QString Service::gql(const QString& name) const {
  return base::readFile(u":/gql/anilist/%1.gql"_s.arg(name));
}

void Service::handleError(const QRestReply& reply) const {
  // @TODO
}

}  // namespace sync::anilist
