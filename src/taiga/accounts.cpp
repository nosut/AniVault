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

#include "accounts.hpp"

#include "base/string.hpp"
#include "taiga/path.hpp"

namespace taiga {

QString Accounts::fileName() const {
  return u"%1/accounts.json"_s.arg(QString::fromStdString(get_data_path()));
}

////////////////////////////////////////////////////////////////////////////////

std::string Accounts::anilistUsername() const {
  return value("anilist.username").toString().toStdString();
}

std::string Accounts::anilistToken() const {
  return value("anilist.token").toString().toStdString();
}

std::string Accounts::kitsuEmail() const {
  return value("kitsu.email").toString().toStdString();
}

std::string Accounts::kitsuUsername() const {
  return value("kitsu.username").toString().toStdString();
}

std::string Accounts::kitsuPassword() const {
  return value("kitsu.password").toString().toStdString();
}

std::string Accounts::myanimelistUsername() const {
  return value("myanimelist.username").toString().toStdString();
}

std::string Accounts::myanimelistAccessToken() const {
  return value("myanimelist.accessToken").toString().toStdString();
}

std::string Accounts::myanimelistRefreshToken() const {
  return value("myanimelist.refreshToken").toString().toStdString();
}

////////////////////////////////////////////////////////////////////////////////

void Accounts::setAnilistUsername(const std::string& username) const {
  setValue("anilist.username", username);
}

void Accounts::setAnilistToken(const std::string& token) const {
  setValue("anilist.token", token);
}

void Accounts::setKitsuEmail(const std::string& email) const {
  setValue("kitsu.email", email);
}

void Accounts::setKitsuUsername(const std::string& username) const {
  setValue("kitsu.username", username);
}

void Accounts::setKitsuPassword(const std::string& password) const {
  setValue("kitsu.password", password);
}

void Accounts::setMyanimelistUsername(const std::string& username) const {
  setValue("myanimelist.username", username);
}

void Accounts::setMyanimelistAccessToken(const std::string& accessToken) const {
  setValue("myanimelist.accessToken", accessToken);
}

void Accounts::setMyanimelistRefreshToken(const std::string& refreshToken) const {
  setValue("myanimelist.refreshToken", refreshToken);
}

}  // namespace taiga
