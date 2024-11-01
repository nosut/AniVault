add_library(taiga-resources STATIC)

target_sources(taiga-resources PUBLIC
	gql.qrc
	icons.qrc
	styles.qrc
	sql.qrc
)

if (CMAKE_SYSTEM_NAME STREQUAL "Windows")
	target_sources(taiga PRIVATE
		taiga.manifest
		taiga.rc
	)
endif()

target_link_libraries(taiga-resources
	taiga-config
)
