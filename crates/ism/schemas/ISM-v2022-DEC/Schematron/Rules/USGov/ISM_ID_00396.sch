<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00396">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00396][Warning] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [KLM], 
        then [KLM] SHOULD contain [NF]; ensure you have proper release authority from the KLM program.
        
        Human Readable: A USA document containing KLM data is usually NOFORN; ensure you have proper release
        authority from the KLM program.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [KLM] this rule checks that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF] and gives a WARNING if there is no [NF].
    </sch:p>
    <sch:rule id="ISM-ID-00396-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('KLM'))]">
          <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))" flag="warning" role="warning">
              [ISM-ID-00396][Warning]  If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols contains the name token [KLM], 
              then [KLM] SHOULD contain [NF]; ensure you have proper release authority from the KLM program.
              
              Human Readable: A USA document containing KLM data is usually NOFORN; ensure you have proper release
              authority from the KLM program.
        </sch:assert>
    </sch:rule>
</sch:pattern>