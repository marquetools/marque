<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00045">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00045][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a name token starting with [SI-G], then attribute
        @ism:disseminationControls must contain the name token [OC].
        
        Human Readable: A USA document containing Special Intelligence (SI)
        GAMMA compartment data must be marked for ORIGINATOR CONTROLLED 
        dissemination.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token
        starting with [SI-G] this rule ensures that attribute
        @ism:disseminationControls is specified with a value containing the
        token [OC].
    </sch:p>
    <sch:rule id="ISM-ID-00045-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G$'))]">
      <sch:assert test="util:containsAnyOfTheTokens(@ism:disseminationControls, ('OC'))" flag="error" role="error">
          [ISM-ID-00045][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
          contains a name token starting with [SI-G], then attribute
          @ism:disseminationControls must contain the name token [OC].
          
          Human Readable: A USA document containing Special Intelligence (SI)
          GAMMA compartment data must be marked for ORIGINATOR CONTROLLED 
          dissemination.
        </sch:assert>
    </sch:rule>
</sch:pattern>