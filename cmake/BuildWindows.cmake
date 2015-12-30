

INSTALL( DIRECTORY ${PROJECT_SOURCE_DIR}/www DESTINATION ${CMAKE_INSTALL_PREFIX} )
INSTALL( DIRECTORY ${PROJECT_SOURCE_DIR}/distfiles/linux/etc DESTINATION ${CMAKE_INSTALL_PREFIX} )

install(FILES ${PROJECT_BINARY_DIR}/third_party/olsr/src/olsr/olsrd DESTINATION bin
	PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)

install(FILES ${PROJECT_BINARY_DIR}/third_party/olsr/src/olsr/lib/olsrd_qaul/olsrd_qaul.so.0.1 DESTINATION lib
	PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)

# generate the CMakeLists.txt for native Windows build
configure_file (
  "${PROJECT_SOURCE_DIR}/src/client/win/CMakeLists.txt.in"
  "${PROJECT_BINARY_DIR}/src/client/win/CMakeLists.txt"
  @ONLY
)

include(cmake/PacketFormatGuesser.cmake)

if(PKGFORMAT MATCHES "AUTO")
  SET(CPACK_GENERATOR ${SPECIFIC_SYSTEM_PREFERED_CPACK_GENERATOR})
else()
  SET(CPACK_GENERATOR ${PKGFORMAT})
endif()

SET(CPACK_SET_DESTDIR ON)

SET(CPACK_DEBIAN_PACKAGE_MAINTAINER "qaul.net community <contact@qaul.net>") #required

# All install must be done before this
INCLUDE(CPack)
