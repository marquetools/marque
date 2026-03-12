<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron"  id="ISM-ID-00335">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText" >
        [ISM-ID-00335][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-O],
        then attribute @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document with HCS-OPERATIONS compartment data must be marked for 
        ORIGINATOR CONTROLLED dissemination.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, for each element which
      specifies attribute @ism:SCIcontrols with a value containing the token
      [HCS-O], this rule ensures that attribute @ism:disseminationControls is
      specified with a value containing the token [OC].
    </sch:p>
    <sch:rule id="ISM-ID-00335-R1" context="*[$ISM_USGOV_RESOURCE  and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('HCS-O'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))" flag="error" role="error">
            [ISM-ID-00335][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [HCS-O],
            then attribute @ism:disseminationControls must contain the name token [OC].
            
            Human Readable: A USA document with HCS-OPERATIONS compartment data must be marked for 
            ORIGINATOR CONTROLLED dissemination.
        </sch:assert>
    </sch:rule>
</sch:pattern>