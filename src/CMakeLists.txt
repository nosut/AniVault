include_directories(${CMAKE_CURRENT_SOURCE_DIR})

add_executable(taiga)

target_sources(taiga PRIVATE
	base/chrono.cpp
	base/xml.cpp
	compat/anime.cpp
	compat/list.cpp
	compat/settings.cpp
	media/anime_db.cpp
	media/anime_season.cpp
	taiga/application.cpp
	taiga/orange.cpp
	taiga/path.cpp
	taiga/settings.cpp
	taiga/version.cpp
	main.cpp
)

target_link_libraries(taiga PRIVATE
	Qt6::Core
	Qt6::Gui
	Qt6::Widgets
	taiga-config
	taiga-deps
	taiga-gui
	taiga-resources
)

if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
	set_target_properties(taiga PROPERTIES OUTPUT_NAME Taiga)
	set_target_properties(taiga PROPERTIES WIN32_EXECUTABLE ON)
endif()

add_subdirectory(gui)
add_subdirectory(resources)
