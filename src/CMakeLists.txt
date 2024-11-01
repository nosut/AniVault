include(QtSetup)

include_directories(${CMAKE_CURRENT_SOURCE_DIR})

add_executable(taiga)

target_sources(taiga PRIVATE
	base/chrono.cpp
	base/chrono.hpp
	base/file.cpp
	base/file.hpp
	base/log.hpp
	base/preprocessor.h
	base/rss.hpp
	base/settings.cpp
	base/settings.hpp
	base/string.cpp
	base/string.hpp
	base/xml.cpp
	base/xml.hpp

	compat/anime.cpp
	compat/anime.hpp
	compat/common.hpp
	compat/list.cpp
	compat/list.hpp
	compat/settings.cpp
	compat/settings.hpp

	media/anime_db.cpp
	media/anime_db.hpp
	media/anime_list.hpp
	media/anime_season.cpp
	media/anime_season.hpp
	media/anime.hpp

	taiga/accounts.cpp
	taiga/accounts.hpp
	taiga/application.cpp
	taiga/application.hpp
	taiga/config.h
	taiga/orange.cpp
	taiga/orange.hpp
	taiga/path.cpp
	taiga/path.hpp
	taiga/session.cpp
	taiga/session.hpp
	taiga/settings.cpp
	taiga/settings.hpp
	taiga/version.cpp
	taiga/version.hpp

	main.cpp
)

target_link_libraries(taiga PRIVATE
	Qt6::Core
	Qt6::Gui
	Qt6::Sql
	Qt6::Widgets
	taiga-config
	taiga-deps
	taiga-gui
	taiga-resources
)

if (TAIGA_PORTABLE)
	target_compile_definitions(taiga PRIVATE TAIGA_PORTABLE)
endif()

if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
	set_target_properties(taiga PROPERTIES OUTPUT_NAME Taiga)
	set_target_properties(taiga PROPERTIES WIN32_EXECUTABLE ON)
endif()

add_subdirectory(gui)
add_subdirectory(resources)

qt_add_translations(taiga
	SOURCE_TARGETS taiga-gui
	TS_FILE_BASE taiga
	TS_FILE_DIR resources/translations
	LUPDATE_OPTIONS -no-obsolete -locations none
)
