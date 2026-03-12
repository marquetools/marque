<?xml version="1.0" encoding="UTF-8"?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<?ICEA pattern?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00213">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00213][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [DISPLAYONLY], then 
        attribute @ism:displayOnlyTo must be specified.
        
        Human Readable: A USA document with DISPLAY ONLY dissemination must 
        indicate the countries to which it may be disclosed.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [DISPLAYONLY] this rule ensures that attribute @ism:displayOnlyTo
    	is specified.
    </sch:p>
  <sch:rule id="ISM-ID-00213-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('DISPLAYONLY'))]">
        <sch:assert test="@ism:displayOnlyTo" flag="error" role="error">
            [ISM-ID-00213][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [DISPLAYONLY], then 
            attribute @ism:displayOnlyTo must be specified.
            
            Human Readable: A USA document with DISPLAY ONLY dissemination must 
            indicate the countries to which it may be disclosed.
        </sch:assert>
    </sch:rule>
</sch:pattern>