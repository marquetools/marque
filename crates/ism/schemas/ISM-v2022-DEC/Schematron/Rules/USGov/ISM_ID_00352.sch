<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00352" is-a="NtkHasCorrespondingDataTwoTokens">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00352][Error] Banner or portion that contributes to roll-up must contain [PR] if PROPIN NTK metadata exists.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Invokes abstract rule NtkHasCorrespondingDataTwoTokens.
   </sch:p>
   <sch:param name="ruleId" value="'ISM-ID-00352'"/>
   <sch:param name="policyName" value="'PROPIN'"/>
   <sch:param name="uriPrefix" value="'urn:us:gov:ic:aces:ntk:propin:'"/>
   <sch:param name="attr" value="'disseminationControls or cuiBasic or cuiSpecified'"/>
   <sch:param name="dataType1" value="'PR'"/>
   <sch:param name="dataType2" value="'PROPIN'"/>
   <sch:param name="dataTokenList" value="($partDisseminationControls_tok,$partCuiBasic_tok,$partCuiSpecified_tok)"/>
   <sch:param name="bannerTokenList" value="($bannerDisseminationControls_tok,$bannerCuiBasic_tok,$bannerCuiSpecified_tok)"/>
</sch:pattern>
