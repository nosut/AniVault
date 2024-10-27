add_library(taiga-deps STATIC)

target_link_libraries(taiga-deps
	anitomy
	monolog
	nstd
	semaver
	taiga-config
)

add_subdirectory(anitomy)
add_subdirectory(monolog)
add_subdirectory(nstd)
add_subdirectory(semaver)
