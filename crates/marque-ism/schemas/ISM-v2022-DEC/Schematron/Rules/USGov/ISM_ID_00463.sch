<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00463">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00463][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [BUR], then attribute @ism:disseminationControls
        must contain the name token [NF].
        
        Human Readable: A USA document containing BUR data must be marked
        for NO FOREIGN dissemination.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [BUR] this rule ensures that attribute @ism:disseminationControls is 
        specified with a value containing the token [NF].
    </sch:p>
    <sch:rule id="ISM-ID-00463-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('BUR'))]">
          <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF'))" flag="error" role="error">
              [ISM-ID-00463][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
              contains the name token [BUR], then attribute @ism:disseminationControls
              must contain the name token [NF].
              
              Human Readable: A USA document containing BUR data must be marked
              for NO FOREIGN dissemination.
        </sch:assert>
    </sch:rule>
</sch:pattern>
