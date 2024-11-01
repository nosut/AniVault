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

#include <windows/win/string.h>

#include "sync/anilist.h"

#include "base/format.h"
#include "base/json.h"
#include "base/log.h"
#include "base/string.h"
#include "media/anime_db.h"
#include "media/anime_item.h"
#include "media/anime_season.h"
#include "media/anime_season_db.h"
#include "media/anime_util.h"
#include "media/library/queue.h"
#include "sync/anilist_util.h"
#include "sync/sync.h"
#include "taiga/http.h"
#include "taiga/settings.h"
#include "track/recognition.h"
#include "ui/translate.h"
#include "ui/ui.h"

namespace sync::anilist {

constexpr auto kRepeatingMediaListStatus = "REPEATING";

////////////////////////////////////////////////////////////////////////////////

bool authenticated_ = false;

bool IsUserAuthenticated() {
  return account.authenticated();
}

void InvalidateUserAuthentication() {
  account.set_authenticated(false);
}

////////////////////////////////////////////////////////////////////////////////

int ParseMediaListObject(const Json& json) {
  const auto anime_id = JsonReadInt(json["media"], "id");
  const auto library_id = JsonReadInt(json, "id");

  if (!anime_id) {
    LOGW(L"Could not parse library entry #{}", library_id);
    return anime::ID_UNKNOWN;
  }

  ParseMediaObject(json["media"]);

  auto& anime_item = anime::db.items[anime_id];

  anime_item.AddtoUserList();
  anime_item.SetMyId(ToWstr(library_id));

  const auto status = JsonReadStr(json, "status");
  if (status == kRepeatingMediaListStatus) {
    anime_item.SetMyStatus(anime::MyStatus::Watching);
    anime_item.SetMyRewatching(true);
  } else {
    anime_item.SetMyStatus(TranslateMyStatusFrom(status));
  }

  anime_item.SetMyScore(JsonReadInt(json, "score"));
  anime_item.SetMyLastWatchedEpisode(JsonReadInt(json, "progress"));
  anime_item.SetMyPrivate(JsonReadBool(json, "private"));
  anime_item.SetMyRewatchedTimes(JsonReadInt(json, "repeat"));
  anime_item.SetMyNotes(StrToWstr(JsonReadStr(json, "notes")));
  anime_item.SetMyDateStart(TranslateFuzzyDateFrom(json["startedAt"]));
  anime_item.SetMyDateEnd(TranslateFuzzyDateFrom(json["completedAt"]));
  anime_item.SetMyLastUpdated(ToWstr(JsonReadInt(json, "updatedAt")));

  return anime_id;
}

void ParseUserObject(const Json& json) {
  Account::set_username(JsonReadStr(json, "name"));
  Account::set_rating_system(
      JsonReadStr(json["mediaListOptions"], "scoreFormat"));
  account.set_authenticated(true);
}

bool HasError(const taiga::http::Response& response) {
  if (response.error()) {
    LOGE(StrToWstr(response.error().str()));
    ui::ChangeStatusText(
        L"AniList: {}"_format(StrToWstr(response.error().str())));
    return true;
  }

  if (response.status_code() == 200)
    return false;

  if (taiga::http::util::IsDdosProtectionEnabled(response)) {
    const std::wstring error_description =
        L"AniList: Cannot connect to server because "
        L"of DDoS protection (Server: {})"_format(
            StrToWstr(response.header("server")));
    LOGE(error_description);
    ui::ChangeStatusText(error_description);
    return true;
  }

  if (Json root; JsonParseString(response.body(), root)) {
    if (root.count("errors")) {
      if (const auto& errors = root["errors"]; errors.is_array()) {
        for (const auto& error : errors) {
          auto error_description = StrToWstr(JsonReadStr(error, "message"));
          if (error.count("validation")) {
            const auto& validation = error["validation"];
            error_description = L"Validation error: " +
                StrToWstr(validation.front().front().get<std::string>());
          }
          if (!error_description.empty()) {
            if (error_description == L"Invalid token") {
              error_description =
                  L"Access token has expired. "
                  L"Please re-authorize your account via Settings.";
            }
            LOGE(error_description);
            ui::ChangeStatusText(L"AniList: {}"_format(error_description));
            return true;
          }
        }
      }
    }
  }

  return false;
}

////////////////////////////////////////////////////////////////////////////////

void AuthenticateUser() {
  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kAuthenticateUser, nullptr));

  const auto on_transfer = [](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::AuthenticateUser, transfer,
                      L"AniList: Authenticating user...");
  };

  const auto on_response = [](const taiga::http::Response& response) {
    if (HasError(response)) {
      account.set_authenticated(false);
      sync::OnError(RequestType::AuthenticateUser);
      return;
    }

    Json root;

    if (!JsonParseString(response.body(), root)) {
      account.set_authenticated(false);
      ui::ChangeStatusText(L"AniList: Could not parse authentication data.");
      sync::OnError(RequestType::AuthenticateUser);
      return;
    }

    ParseUserObject(root["data"]["Viewer"]);

    sync::OnResponse(RequestType::AuthenticateUser);

    if (account.authenticated()) {
      sync::Synchronize();
    } else {
      GetLibraryEntries();
    }
  };

  taiga::http::Send(request, on_transfer, on_response);
}

void GetLibraryEntries() {
  const Json variables{
    {"userName", Account::username()},
  };

  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kGetLibraryEntries, variables));

  const auto on_transfer = [](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::GetLibraryEntries, transfer,
                      L"AniList: Retrieving anime list...");
  };

  const auto on_response = [](const taiga::http::Response& response) {
    if (HasError(response)) {
      sync::OnError(RequestType::GetLibraryEntries);
      return;
    }

    Json root;

    if (!JsonParseString(response.body(), root)) {
      ui::ChangeStatusText(L"AniList: Could not parse anime list.");
      sync::OnError(RequestType::GetLibraryEntries);
      return;
    }

    anime::db.ClearUserData();

    const auto& lists = root["data"]["MediaListCollection"]["lists"];
    for (const auto& list : lists) {
      const auto& entries = list["entries"];
      for (const auto& entry : entries) {
        ParseMediaListObject(entry);
      }
    }

    sync::OnResponse(RequestType::GetLibraryEntries);
  };

  taiga::http::Send(request, on_transfer, on_response);
}

void GetMetadataById(const int id) {
  const Json variables{
    {"id", id},
  };

  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kGetMetadataById, variables));

  const auto on_transfer = [](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::GetMetadataById, transfer,
                      L"AniList: Retrieving anime information...");
  };

  const auto on_response = [id](const taiga::http::Response& response) {
    if (HasError(response)) {
      if (response.status_code() == 404) {
        sync::OnInvalidAnimeId(id);
      } else {
        ui::OnLibraryEntryChangeFailure(id);
      }
      sync::OnError(RequestType::GetMetadataById);
      return;
    }

    Json root;

    if (!JsonParseString(response.body(), root)) {
      ui::ChangeStatusText(L"AniList: Could not parse anime data.");
      ui::OnLibraryEntryChangeFailure(id);
      sync::OnError(RequestType::GetMetadataById);
      return;
    }

    const int anime_id = ParseMediaObject(root["data"]["Media"]);

    ui::OnLibraryEntryChange(anime_id);
    sync::OnResponse(RequestType::GetMetadataById);
  };

  taiga::http::Send(request, on_transfer, on_response);
}

void GetSeason(const anime::Season season, const int page) {
  const Json variables{
    {"season", TranslateSeasonTo(ui::TranslateSeasonName(season.name))},
    {"seasonYear", static_cast<int>(season.year)},
    {"page", page},
  };

  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kGetSeason, variables));

  const auto on_transfer = [season](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::GetSeason, transfer,
        L"AniList: Retrieving {} anime season..."_format(
            ui::TranslateSeason(season)));
  };

  const auto on_response = [season](const taiga::http::Response& response) {
    if (HasError(response)) {
      sync::OnError(RequestType::GetSeason);
      return;
    }

    Json root;

    if (!JsonParseString(response.body(), root)) {
      ui::ChangeStatusText(L"AniList: Could not parse season data.");
      sync::OnError(RequestType::GetSeason);
      return;
    }

    const auto& page = root["data"]["Page"];

    const auto& page_info = page["pageInfo"];
    const int current_page = JsonReadInt(page_info, "currentPage");
    const bool has_next_page = JsonReadBool(page_info, "hasNextPage");

    if (current_page <= 1) {  // first page
      anime::season_db.items.clear();
    }

    for (const auto& media : page["media"]) {
      const auto anime_id = ParseMediaObject(media);
      anime::season_db.items.push_back(anime_id);
      ui::OnLibraryEntryChange(anime_id);
    }

    if (has_next_page) {
      GetSeason(season, current_page + 1);
    } else {
      sync::OnResponse(RequestType::GetSeason);
    }
  };

  taiga::http::Send(request, on_transfer, on_response);
}

void SearchTitle(const std::wstring& title) {
  const Json variables{
    {"query", WstrToStr(title)},
  };

  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kSearchTitle, variables));

  const auto on_transfer = [title](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::SearchTitle, transfer,
                      L"AniList: Searching for \"{}\"..."_format(title));
  };

  const auto on_response = [](const taiga::http::Response& response) {
    if (HasError(response)) {
      sync::OnError(RequestType::SearchTitle);
      return;
    }

    Json root;

    if (!JsonParseString(response.body(), root)) {
      ui::ChangeStatusText(L"AniList: Could not parse search results.");
      sync::OnError(RequestType::SearchTitle);
      return;
    }

    std::vector<int> ids;
    for (const auto& media : root["data"]["Page"]["media"]) {
      const auto anime_id = ParseMediaObject(media);
      ids.push_back(anime_id);
    }

    ui::OnLibrarySearchTitle(ids);
    sync::OnResponse(RequestType::SearchTitle);
  };

  taiga::http::Send(request, on_transfer, on_response);
}

void AddLibraryEntry(const library::QueueItem& queue_item) {
  UpdateLibraryEntry(queue_item);
}

void DeleteLibraryEntry(const int id) {
  const auto anime_item = anime::db.Find(id);
  const auto library_id = ToInt(anime_item->GetMyId());

  const Json variables{
    {"id", library_id},
  };

  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kDeleteLibraryEntry, variables));

  const auto on_transfer = [](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::DeleteLibraryEntry, transfer,
                      L"AniList: Deleting anime from list...");
  };

  const auto on_response = [id](const taiga::http::Response& response) {
    if (HasError(response)) {
      if (response.status_code() == 404) {
        // We consider "404 Not Found" to be a success.
      } else {
        ui::OnLibraryUpdateFailure(id, StrToWstr(response.error().str()),
                                   false);
        sync::OnError(RequestType::DeleteLibraryEntry);
        return;
      }
    }

    // Returns: {"data":{"DeleteMediaListEntry":{"deleted":true}}}

    sync::OnResponse(RequestType::DeleteLibraryEntry);
  };

  taiga::http::Send(request, on_transfer, on_response);
}

void UpdateLibraryEntry(const library::QueueItem& queue_item) {
  const auto id = queue_item.anime_id;

  Json variables{
    {"mediaId", queue_item.anime_id},
  };

  if (const auto anime_item = anime::db.Find(queue_item.anime_id)) {
    if (const auto library_id = ToInt(anime_item->GetMyId())) {
      variables["id"] = library_id;
    }
  }

  if (queue_item.enable_rewatching && *queue_item.enable_rewatching) {
    variables["status"] = kRepeatingMediaListStatus;
  } else if (queue_item.status) {
    variables["status"] = TranslateMyStatusTo(*queue_item.status);
  }
  if (queue_item.score)
    variables["scoreRaw"] = *queue_item.score;
  if (queue_item.episode)
    variables["progress"] = *queue_item.episode;
  if (queue_item.rewatched_times)
    variables["repeat"] = *queue_item.rewatched_times;
  if (queue_item.notes)
    variables["notes"] = WstrToStr(*queue_item.notes);
  if (queue_item.date_start)
    variables["startedAt"] = TranslateFuzzyDateTo(*queue_item.date_start);
  if (queue_item.date_finish)
    variables["completedAt"] = TranslateFuzzyDateTo(*queue_item.date_finish);

  auto request = BuildRequest();
  request.set_body(BuildRequestBody(gql::kUpdateLibraryEntry, variables));

  const auto on_transfer = [](const taiga::http::Transfer& transfer) {
    return OnTransfer(RequestType::UpdateLibraryEntry, transfer,
                      L"AniList: Updating anime list...");
  };

  const auto on_response = [id](const taiga::http::Response& response) {
    if (HasError(response)) {
      auto error_description = StrToWstr(response.error().str());
      if (response.status_code() == 404) {
        error_description = L"AniList: Anime list entry does not exist.";
      }
      ui::OnLibraryUpdateFailure(id, error_description, false);
      sync::OnError(RequestType::UpdateLibraryEntry);
      return;
    }

    Json root;

    if (!JsonParseString(response.body(), root)) {
      ui::ChangeStatusText(L"AniList: Could not parse anime list entry.");
      sync::OnError(RequestType::UpdateLibraryEntry);
      return;
    }

    ParseMediaListObject(root["data"]["SaveMediaListEntry"]);

    sync::OnResponse(RequestType::UpdateLibraryEntry);
  };

  taiga::http::Send(request, on_transfer, on_response);
}

}  // namespace sync::anilist
