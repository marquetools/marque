<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00302">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00302][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [OC-USGOV], then 
        name token [OC] must be specified.
        
        Human Readable: A USA document with OC-USGOV dissemination must 
        also contain an OC dissemination.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [OC-USGOV], this rule ensures that token [OC] is also specified.
    </sch:p>
  <sch:rule id="ISM-ID-00302-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC-USGOV'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))" flag="error" role="error">
            [ISM-ID-00302][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [OC-USGOV], then 
            name token [OC] must be specified.
            
            Human Readable: A USA document with OC-USGOV dissemination must 
            also contain an OC dissemination.
        </sch:assert>
    </sch:rule>
</sch:pattern>