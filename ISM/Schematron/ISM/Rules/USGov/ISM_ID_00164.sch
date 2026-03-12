<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00164">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00164][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:disseminationControls contains the name token [RS],
        then attribute @ism:classification must have a value of [TS] or [S].
        
        Human Readable: USA documents with RISK SENSITIVE dissemination must
        be classified SECRET or TOP SECRET.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
    	If the document is an ISM_USGOV_RESOURCE, for each element which has 
    	attribute @ism:disseminationControls specified with a value containing
    	the token [RS] this rule ensures that attribute @ism:classification is not
    	specified with a value of [TS] or [S].
    </sch:p>
  <sch:rule id="ISM-ID-00164-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:disseminationControls, ('RS'))]">
        <sch:assert test="@ism:classification=('TS', 'S')" flag="error" role="error">
            [ISM-ID-00164][Error] If ISM_USGOV_RESOURCE and attribute 
            @ism:disseminationControls contains the name token [RS],
            then attribute @ism:classification must have a value of [TS] or [S].
            
            Human Readable: USA documents with RISK SENSITIVE dissemination must
            be classified SECRET or TOP SECRET.
        </sch:assert>
    </sch:rule>
</sch:pattern>