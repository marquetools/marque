<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00168">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00168][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls is not specified or is specified and does not contain the name token 
        [DISPLAYONLY], then attribute @ism:displayOnlyTo must not be specified.
        
        Human Readable: If a portion in a USA document is not marked for DISPLAY ONLY dissemination, 
        it must not list countries to which it may be disclosed. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE and attribute @ism:disseminationControls
        does not contain the token [DISPLAYONLY], this rule ensures that the attribute 
      	@ism:displayOnlyTo is not specified.
    </sch:p>
	  <sch:rule id="ISM-ID-00168-R1" context="*[$ISM_USGOV_RESOURCE and not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY')))]">
        <sch:assert test="not(@ism:displayOnlyTo)" flag="error" role="error">
            [ISM-ID-00168][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls is not specified or is specified and does not contain the name token 
            [DISPLAYONLY], then attribute @ism:displayOnlyTo must not be specified.
            
            Human Readable: If a portion in a USA document is not marked for DISPLAY ONLY dissemination, 
            it must not list countries to which it may be disclosed. 
        </sch:assert>
    </sch:rule>
</sch:pattern>