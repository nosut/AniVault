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

#pragma once

#include <QString>
#include <string>

#include "base/settings.hpp"

namespace taiga {

class Accounts final : public base::Settings {
public:
  std::string anilistUsername() const;
  std::string anilistToken() const;

  std::string kitsuEmail() const;
  std::string kitsuUsername() const;
  std::string kitsuPassword() const;

  std::string myanimelistUsername() const;
  std::string myanimelistAccessToken() const;
  std::string myanimelistRefreshToken() const;

  void setAnilistUsername(const std::string& username) const;
  void setAnilistToken(const std::string& token) const;

  void setKitsuEmail(const std::string& email) const;
  void setKitsuUsername(const std::string& username) const;
  void setKitsuPassword(const std::string& password) const;

  void setMyanimelistUsername(const std::string& username) const;
  void setMyanimelistAccessToken(const std::string& accessToken) const;
  void setMyanimelistRefreshToken(const std::string& refreshToken) const;

  std::string serviceUsername(const std::string& service) const;

private:
  QString fileName() const override;
};

inline Accounts accounts;

}  // namespace taiga
