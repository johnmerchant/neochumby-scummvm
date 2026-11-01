# Included after ScummVM's generated Makefile: our backend, built as its files
# are, in place of backends/platform/null/null.o, and the list of what the
# final link (build.rs) takes.
CHUMBY_DIR := $(dir $(lastword $(MAKEFILE_LIST)))
CHUMBY_OBJS := $(filter-out backends/platform/null/null.o,$(DETECT_OBJS) $(OBJS))

chumby.o: $(CHUMBY_DIR)chumby.cpp config.h config.mk
	$(CXX) $(CXXFLAGS) $(CPPFLAGS) $(DEFINES) $(INCLUDES) -c $< -o $@

chumby-all: chumby.o $(CHUMBY_OBJS)
	@echo $(CHUMBY_OBJS) > chumby-objs.txt
